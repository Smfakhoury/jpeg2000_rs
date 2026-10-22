use super::ht_reader_error;
use vstd::prelude::*;

verus! {

pub struct Part1PacketGrid {
    pub layers: u16,
    pub components: u16,
    pub resolutions: u8,
    pub component_resolutions: Seq<u8>,
    pub maximum_precincts_per_resolution: u32,
    pub precinct_counts: Seq<u32>,
    pub precinct_offsets: Seq<usize>,
    pub candidate_count: u64,
    pub packet_count: u64,
}

pub struct Part1PacketIterator {
    pub grid: Part1PacketGrid,
    pub order: crate::codestream::ProgressionOrder,
    pub candidate_index: u64,
    pub emitted: u64,
}

pub struct Part1ProgressionPacketIterator {
    pub grid: Part1PacketGrid,
    pub changes: Seq<crate::codestream::ProgressionChange>,
    pub default: Option<Part1PacketIterator>,
    pub window_index: usize,
    pub window_candidate: u64,
    pub window_candidates: u64,
    pub emitted: u64,
}

#[verifier::opaque]
pub open spec fn part1_progression_admission(
    grid: Part1PacketGrid, records: Seq<crate::codestream::ProgressionChange>,
    index: usize, work: u64, maximum: u64,
) -> Result<(), crate::DecodeError>
    decreases records.len() - index as nat,
{
    if index >= records.len() { Ok(()) }
    else {
        match part1_packet_window_count(grid, records[index as int]) {
            Err(error) => Err(error),
            Ok(amount) => if work as nat + amount as nat > u64::MAX {
                Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow))
            } else if work as nat + amount as nat > maximum {
                Err(super::part1_packet_reader_limit(crate::Resource::WorkUnits, maximum, (work + amount) as u64))
            } else if index == usize::MAX { Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow)) }
            else { part1_progression_admission(grid, records, (index+1) as usize, (work+amount) as u64, maximum) },
        }
    }
}

pub open spec fn part1_progression_iterator_initial(
    grid: Part1PacketGrid, order: crate::codestream::ProgressionOrder,
    records: Seq<crate::codestream::ProgressionChange>, maximum_work: u64,
) -> Result<Part1ProgressionPacketIterator, crate::DecodeError> {
    match part1_progression_admission(grid, records, 0, 0, maximum_work) {
        Err(error) => Err(error),
        Ok(()) => {
            let first = if records.len() == 0 { Ok(0u64) } else { part1_packet_window_count(grid, records[0]) };
            match first {
                Err(error) => Err(error),
                Ok(count) => Ok(Part1ProgressionPacketIterator {
                    grid, changes:records, default:if records.len()==0 {
                        Some(Part1PacketIterator { grid, order, candidate_index:0, emitted:0 })
                    } else { None },
                    window_index:0, window_candidate:0, window_candidates:count, emitted:0,
                }),
            }
        },
    }
}

#[verifier::opaque]
pub open spec fn part1_progression_prior_contains(
    records: Seq<crate::codestream::ProgressionChange>, end: usize, index: usize,
    coordinate: crate::packet::PacketCoordinate,
) -> Result<bool, crate::DecodeError>
    decreases if index >= end { 0nat } else { (end-index) as nat },
{
    if index >= end { Ok(false) }
    else if index >= records.len() { Err(ht_reader_error(crate::MalformedKind::InvalidProgressionOrderChange)) }
    else if crate::codestream::progression_change_contains(records[index as int],
        coordinate.layer, coordinate.resolution, coordinate.component) { Ok(true) }
    else if index == usize::MAX { Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow)) }
    else { part1_progression_prior_contains(records, end, (index+1) as usize, coordinate) }
}

pub open spec fn part1_progression_candidate_step(
    state: Part1ProgressionPacketIterator, record: crate::codestream::ProgressionChange,
) -> (Result<Option<crate::packet::PacketCoordinate>,crate::DecodeError>,Part1ProgressionPacketIterator) {
    if state.window_candidate == u64::MAX { (Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow)),state) }
    else {
        let candidate = state.window_candidate;
        let state = Part1ProgressionPacketIterator { window_candidate:(candidate+1) as u64, ..state };
        match part1_packet_window_candidate(state.grid, record, candidate) {
            Err(error) => (Err(error),state),
            Ok(coordinate) => match part1_packet_grid_precinct_count(state.grid, coordinate.component, coordinate.resolution) {
                Err(error) => (Err(error),state),
                Ok(count) => if coordinate.precinct >= count { (Ok(None),state) }
                else {
                    match part1_progression_prior_contains(state.changes, state.window_index, 0, coordinate) {
                        Err(error) => (Err(error),state),
                        Ok(true) => (Ok(None),state),
                        Ok(false) => if state.emitted == u64::MAX { (Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow)),state) }
                        else {
                            let state = Part1ProgressionPacketIterator { emitted:(state.emitted+1) as u64, ..state };
                            if state.emitted > state.grid.packet_count {
                                (Err(ht_reader_error(crate::MalformedKind::InvalidProgressionOrderChange)),state)
                            } else { (Ok(Some(coordinate)),state) }
                        },
                    }
                },
            },
        }
    }
}

pub open spec fn part1_progression_next_window(state: Part1ProgressionPacketIterator)
    -> (Result<(),crate::DecodeError>,Part1ProgressionPacketIterator) {
    if state.window_index == usize::MAX { (Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow)),state) }
    else {
        let state = Part1ProgressionPacketIterator { window_index:(state.window_index+1) as usize, window_candidate:0, ..state };
        let next = if state.window_index < state.changes.len() {
            part1_packet_window_count(state.grid, state.changes[state.window_index as int])
        } else { Ok(0u64) };
        match next {
            Err(error) => (Err(error),state),
            Ok(count) => (Ok(()),Part1ProgressionPacketIterator { window_candidates:count, ..state }),
        }
    }
}

#[verifier::opaque]
pub open spec fn part1_progression_windows_next(state: Part1ProgressionPacketIterator)
    -> (Result<Option<crate::packet::PacketCoordinate>,crate::DecodeError>,Part1ProgressionPacketIterator)
    decreases state.changes.len() - state.window_index as nat,
        if state.window_candidate >= state.window_candidates { 0nat }
        else { (state.window_candidates-state.window_candidate) as nat },
{
    if state.window_index >= state.changes.len() {
        if state.emitted != state.grid.packet_count { (Err(ht_reader_error(crate::MalformedKind::InvalidProgressionOrderChange)),state) }
        else { (Ok(None),state) }
    } else if state.window_candidate < state.window_candidates {
        let step = part1_progression_candidate_step(state,state.changes[state.window_index as int]);
        match step.0 { Ok(None) => part1_progression_windows_next(step.1), _ => step }
    } else {
        let step = part1_progression_next_window(state);
        match step.0 { Ok(()) => part1_progression_windows_next(step.1), Err(error) => (Err(error),step.1) }
    }
}

pub open spec fn part1_progression_iterator_next(state: Part1ProgressionPacketIterator)
    -> (Result<Option<crate::packet::PacketCoordinate>,crate::DecodeError>,Part1ProgressionPacketIterator) {
    match state.default {
        None => part1_progression_windows_next(state),
        Some(default) => {
            let step = part1_packet_iterator_next(default);
            let state = Part1ProgressionPacketIterator { default:Some(step.1), ..state };
            match step.0 {
                Err(error) => (Err(error),state),
                Ok(coordinate) => (Ok(coordinate),Part1ProgressionPacketIterator { emitted:step.1.emitted, ..state }),
            }
        },
    }
}

#[verifier::opaque]
pub open spec fn part1_grid_resolution_summary(
    values: Seq<u8>, index: usize, maximum: u8, zero: bool,
) -> Result<(u8,bool), crate::DecodeError>
    decreases values.len() - index as nat,
{
    if index >= values.len() { Ok((maximum,zero)) }
    else if index == usize::MAX { Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow)) }
    else { part1_grid_resolution_summary(values, (index+1) as usize,
        if values[index as int] > maximum { values[index as int] } else { maximum },
        zero || values[index as int] == 0) }
}

pub open spec fn part1_packet_grid_total_precincts(grid: Part1PacketGrid) -> Result<u64, crate::DecodeError> {
    if grid.layers == 0 { Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow)) }
    else { Ok((grid.packet_count / grid.layers as u64) as u64) }
}

pub open spec fn part1_packet_grid_precinct_count(
    grid: Part1PacketGrid, component: u16, resolution: u8,
) -> Result<u32, crate::DecodeError> {
    if component >= grid.components || resolution >= grid.resolutions
        || component as nat >= grid.component_resolutions.len() {
        Err(ht_reader_error(crate::MalformedKind::InvalidCodingStyle))
    } else if resolution >= grid.component_resolutions[component as int] {
        Ok(0)
    } else if component as nat * grid.resolutions as nat + resolution as nat > usize::MAX {
        Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow))
    } else {
        let index = component as int * grid.resolutions as int + resolution as int;
        if index >= grid.precinct_counts.len() { Err(ht_reader_error(crate::MalformedKind::InvalidCodingStyle)) }
        else { Ok(grid.precinct_counts[index]) }
    }
}

pub open spec fn part1_packet_grid_precinct_index(
    grid: Part1PacketGrid, component: u16, resolution: u8, precinct: u32,
) -> Result<usize, crate::DecodeError> {
    match part1_packet_grid_precinct_count(grid, component, resolution) {
        Err(error) => Err(error),
        Ok(count) => if precinct >= count { Err(ht_reader_error(crate::MalformedKind::InvalidCodingStyle)) }
        else if component as nat * grid.resolutions as nat + resolution as nat > usize::MAX {
            Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow))
        } else {
            let index = component as int * grid.resolutions as int + resolution as int;
            if index >= grid.precinct_offsets.len() { Err(ht_reader_error(crate::MalformedKind::InvalidCodingStyle)) }
            else if precinct as nat > usize::MAX || grid.precinct_offsets[index] as nat + precinct as nat > usize::MAX {
                Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow))
            } else { Ok((grid.precinct_offsets[index] + precinct) as usize) }
        },
    }
}

#[verifier::opaque]
pub open spec fn part1_packet_selected_row(
    grid: Part1PacketGrid, component: u16, resolution: u8, count: u8, total: u64,
) -> Result<u64, crate::DecodeError>
    decreases if resolution >= count { 0nat } else { (count - resolution) as nat },
{
    if resolution >= count { Ok(total) }
    else {
        match part1_packet_grid_precinct_count(grid, component, resolution) {
            Err(error) => Err(error),
            Ok(amount) => if total as nat + amount as nat > u64::MAX || resolution == u8::MAX {
                Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow))
            } else { part1_packet_selected_row(grid, component, (resolution + 1) as u8,
                count, (total + amount) as u64) },
        }
    }
}

#[verifier::opaque]
pub open spec fn part1_packet_selected_count_run(
    grid: Part1PacketGrid, counts: Seq<u8>, component: u16, total: u64,
) -> Result<u64, crate::DecodeError>
    decreases if component >= grid.components { 0nat } else { (grid.components - component) as nat },
{
    if component >= grid.components {
        if total as nat * grid.layers as nat > u64::MAX {
            Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow))
        } else { Ok((total * grid.layers) as u64) }
    } else if component as nat >= counts.len() || counts[component as int] == 0
        || component as nat >= grid.component_resolutions.len()
        || counts[component as int] > grid.component_resolutions[component as int] {
        Err(ht_reader_error(crate::MalformedKind::InvalidCodingStyle))
    } else {
        match part1_packet_selected_row(grid, component, 0, counts[component as int], total) {
            Err(error) => Err(error),
            Ok(total) => if component == u16::MAX {
                Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow))
            } else { part1_packet_selected_count_run(grid, counts, (component + 1) as u16, total) },
        }
    }
}

pub open spec fn part1_packet_grid_selected_count(grid: Part1PacketGrid, counts: Seq<u8>)
    -> Result<u64, crate::DecodeError> {
    if counts.len() != grid.components as nat {
        Err(ht_reader_error(crate::MalformedKind::InvalidCodingStyle))
    } else { part1_packet_selected_count_run(grid, counts, 0, 0) }
}

pub open spec fn part1_packet_unravel(index: u64, dimensions: Seq<u64>) -> Result<Seq<u64>, crate::DecodeError> {
    if dimensions.len() != 4 || dimensions[0] == 0 || dimensions[1] == 0 || dimensions[2] == 0 || dimensions[3] == 0 {
        Err(ht_reader_error(crate::MalformedKind::InvalidCodingStyle))
    } else {
        let third = index % dimensions[3];
        let index = index / dimensions[3];
        let second = index % dimensions[2];
        let index = index / dimensions[2];
        let first = index % dimensions[1];
        let index = index / dimensions[1];
        if index >= dimensions[0] { Err(ht_reader_error(crate::MalformedKind::InvalidCodingStyle)) }
        else { Ok(seq![index as u64, first as u64, second as u64, third as u64]) }
    }
}

pub open spec fn part1_packet_candidate_dimensions(
    grid: Part1PacketGrid, order: crate::codestream::ProgressionOrder,
) -> Seq<u64> {
    let l = grid.layers as u64;
    let r = grid.resolutions as u64;
    let c = grid.components as u64;
    let p = grid.maximum_precincts_per_resolution as u64;
    match order {
        crate::codestream::ProgressionOrder::Lrcp => seq![l,r,c,p],
        crate::codestream::ProgressionOrder::Rlcp => seq![r,l,c,p],
        crate::codestream::ProgressionOrder::Rpcl => seq![r,p,c,l],
        crate::codestream::ProgressionOrder::Pcrl => seq![p,c,r,l],
        crate::codestream::ProgressionOrder::Cprl => seq![c,p,r,l],
    }
}

pub open spec fn part1_packet_candidate(
    grid: Part1PacketGrid, order: crate::codestream::ProgressionOrder, candidate: u64,
) -> Result<crate::packet::PacketCoordinate, crate::DecodeError> {
    match part1_packet_unravel(candidate, part1_packet_candidate_dimensions(grid, order)) {
        Err(error) => Err(error),
        Ok(v) => {
            let (l,r,c,p) = match order {
                crate::codestream::ProgressionOrder::Lrcp => (v[0],v[1],v[2],v[3]),
                crate::codestream::ProgressionOrder::Rlcp => (v[1],v[0],v[2],v[3]),
                crate::codestream::ProgressionOrder::Rpcl => (v[3],v[0],v[2],v[1]),
                crate::codestream::ProgressionOrder::Pcrl => (v[3],v[2],v[1],v[0]),
                crate::codestream::ProgressionOrder::Cprl => (v[3],v[2],v[0],v[1]),
            };
            if l > u16::MAX || r > u8::MAX || c > u16::MAX || p > u32::MAX {
                Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow))
            } else { Ok(crate::packet::PacketCoordinate {
                layer:l as u16, resolution:r as u8, component:c as u16, precinct:p as u32,
            }) }
        },
    }
}

pub open spec fn part1_packet_window_count(grid: Part1PacketGrid, record: crate::codestream::ProgressionChange)
    -> Result<u64, crate::DecodeError> {
    if record.resolution_end < record.resolution_start {
        Err(ht_reader_error(crate::MalformedKind::InvalidProgressionOrderChange))
    } else {
        let first = record.layer_end as nat * (record.resolution_end - record.resolution_start) as nat;
        let components = if record.component_end < record.component_start { 0nat }
            else { (record.component_end - record.component_start) as nat };
        let second = first * components;
        let third = second * grid.maximum_precincts_per_resolution as nat;
        if first > u64::MAX || second > u64::MAX || third > u64::MAX {
            Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow))
        } else { Ok(third as u64) }
    }
}

pub open spec fn part1_packet_window_candidate(
    grid: Part1PacketGrid, record: crate::codestream::ProgressionChange, candidate: u64,
) -> Result<crate::packet::PacketCoordinate, crate::DecodeError> {
    if record.resolution_end < record.resolution_start || record.component_end < record.component_start {
        Err(ht_reader_error(crate::MalformedKind::InvalidProgressionOrderChange))
    } else {
        let local_grid = Part1PacketGrid {
            layers:record.layer_end,
            resolutions:(record.resolution_end - record.resolution_start) as u8,
            components:(record.component_end - record.component_start) as u16,
            ..grid
        };
        match part1_packet_candidate(local_grid, record.progression_order, candidate) {
            Err(error) => Err(error),
            Ok(local) => if local.resolution as nat + record.resolution_start as nat > u8::MAX
                || local.component as nat + record.component_start as nat > u16::MAX {
                Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow))
            } else { Ok(crate::packet::PacketCoordinate {
                resolution:(local.resolution + record.resolution_start) as u8,
                component:(local.component + record.component_start) as u16,
                ..local
            }) },
        }
    }
}

#[verifier::opaque]
pub open spec fn part1_packet_iterator_next(state: Part1PacketIterator)
    -> (Result<Option<crate::packet::PacketCoordinate>, crate::DecodeError>, Part1PacketIterator)
    decreases if state.candidate_index >= state.grid.candidate_count { 0nat }
        else { (state.grid.candidate_count - state.candidate_index) as nat },
{
    if state.candidate_index >= state.grid.candidate_count { (Ok(None), state) }
    else {
        let candidate = state.candidate_index;
        let state = Part1PacketIterator { candidate_index:(candidate + 1) as u64, ..state };
        match part1_packet_candidate(state.grid, state.order, candidate) {
            Err(error) => (Err(error),state),
            Ok(coordinate) => match part1_packet_grid_precinct_count(state.grid, coordinate.component, coordinate.resolution) {
                Err(error) => (Err(error),state),
                Ok(count) => if coordinate.precinct >= count { part1_packet_iterator_next(state) }
                else if state.emitted == u64::MAX { (Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow)),state) }
                else {
                    let state = Part1PacketIterator { emitted:(state.emitted + 1) as u64, ..state };
                    if state.emitted > state.grid.packet_count { (Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow)),state) }
                    else { (Ok(Some(coordinate)),state) }
                },
            },
        }
    }
}

}
