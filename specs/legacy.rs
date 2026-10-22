use super::*;
use vstd::prelude::*;

verus! {

pub enum Part1LegacySymbols {
    Mq(Part1MqState),
    Raw(Part1RawReaderState),
}

pub open spec fn part1_legacy_default_style() -> crate::tier1::Tier1CodingStyle {
    crate::tier1::Tier1CodingStyle {
        block_coding: crate::tier1::Tier1BlockCoding::Legacy, force_high_throughput: false,
        selective_arithmetic_coding_bypass: false, reset_contexts: false, terminate_all_passes: false,
        vertical_causal_context: false, predictable_termination: false, segmentation_symbols: false,
    }
}

#[verifier::opaque]
pub open spec fn part1_legacy_symbol(s: Part1LegacySymbols, context: usize)
    -> (Result<bool, crate::DecodeError>, Part1LegacySymbols)
{
    match s {
        Part1LegacySymbols::Mq(mq) => {
            let next = part1_mq_decode(mq, context);
            (next.0, Part1LegacySymbols::Mq(next.1))
        },
        Part1LegacySymbols::Raw(raw) => {
            let next = part1_raw_reader_read(raw);
            (next.0, Part1LegacySymbols::Raw(next.1))
        },
    }
}

pub struct Part1LegacyBitplane {
    pub width: u16,
    pub height: u16,
    pub bitplane: u16,
    pub significant_before: Seq<bool>,
    pub significance_visited: Seq<bool>,
    pub significance_propagation_complete: bool,
    pub magnitude_refinement_complete: bool,
    pub cleanup_complete: bool,
}

pub open spec fn part1_legacy_bitplane_initial(plane: Tier1RawPlane, bitplane: u16)
    -> Result<Part1LegacyBitplane, crate::DecodeError>
{
    if bitplane >= 292 { Err(ht_reader_error(crate::MalformedKind::InvalidCodeBlock)) }
    else { Ok(Part1LegacyBitplane {
        width: plane.width, height: plane.height, bitplane,
        significant_before: plane.coefficients.map_values(|c: Tier1RawCoefficient| c.significant),
        significance_visited: Seq::new(plane.coefficients.len(), |i: int| false),
        significance_propagation_complete: false, magnitude_refinement_complete: false, cleanup_complete: false,
    }) }
}

pub open spec fn part1_legacy_validate(state: Part1LegacyBitplane, plane: Tier1RawPlane)
    -> Result<(), crate::DecodeError>
{
    if state.width != plane.width || state.height != plane.height
        || state.significant_before.len() != plane.coefficients.len()
        || state.significance_visited.len() != plane.coefficients.len() {
        Err(ht_reader_error(crate::MalformedKind::InvalidCodeBlock))
    } else { Ok(()) }
}

pub open spec fn part1_legacy_zero_context(summary: crate::tier1::NeighborSummary, orientation: crate::tier1::SubbandOrientation) -> u8 {
    let horizontal = if orientation is Hl { summary.vertical_significant } else { summary.horizontal_significant };
    let vertical = if orientation is Hl { summary.horizontal_significant } else { summary.vertical_significant };
    part1_tier1_zero_context(horizontal as u16, vertical as u16, summary.diagonal_significant as u16, orientation is Hh)
}

pub open spec fn part1_legacy_sign_context(summary: crate::tier1::NeighborSummary) -> crate::tier1::SignContext {
    let horizontal = if summary.horizontal_sign_balance < 0 { -1i8 } else if summary.horizontal_sign_balance > 0 { 1i8 } else { 0i8 };
    let vertical = if summary.vertical_sign_balance < 0 { -1i8 } else if summary.vertical_sign_balance > 0 { 1i8 } else { 0i8 };
    let h = if horizontal < 0 { 1i8 } else { horizontal };
    let v = if horizontal < 0 { (-vertical) as i8 } else { vertical };
    crate::tier1::SignContext {
        context: (9 + part1_tier1_sign_offset(h, v)) as u8,
        predicted_negative: part1_tier1_predicted_negative(summary.horizontal_sign_balance, summary.vertical_sign_balance),
    }
}

pub open spec fn part1_legacy_refinement_context(coefficient: Tier1RawCoefficient, summary: crate::tier1::NeighborSummary) -> u8 {
    part1_tier1_refinement_context(coefficient.refined,
        summary.horizontal_significant as nat + summary.vertical_significant as nat + summary.diagonal_significant as nat) as u8
}

pub open spec fn part1_legacy_initialized(s: Part1MqState) -> Part1MqState {
    Part1MqState {
        contexts: Seq::new(19, |i: int| (if i == 0 { 4u8 } else if i == 17 { 3u8 } else if i == 18 { 46u8 } else { 0u8 }, false)),
        ..s
    }
}

pub open spec fn part1_legacy_mark(
    plane: Tier1RawPlane, x: u16, y: u16, bitplane: u16, negative: bool,
) -> (Result<(), crate::DecodeError>, Tier1RawPlane) {
    if bitplane >= 384 {
        (Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow)), plane)
    } else {
        let marked = tier1_mark_coefficient(plane, x, y, part1_magnitude_bit(bitplane), pow2(bitplane as nat), negative);
        match marked.0 {
            Err(error) => (Err(error), marked.1),
            Ok(()) => match tier1_raw_plane_index(marked.1, x, y) {
                Err(error) => (Err(error), marked.1),
                Ok(index) => (Ok(()), Tier1RawPlane {
                    coefficients: marked.1.coefficients.update(index as int,
                        Tier1RawCoefficient { legacy_bitplane: Some(bitplane), ..marked.1.coefficients[index as int] }),
                    ..marked.1
                }),
            },
        }
    }
}

pub open spec fn part1_legacy_refine(
    plane: Tier1RawPlane, x: u16, y: u16, bitplane: u16, bit: bool,
) -> (Result<(), crate::DecodeError>, Tier1RawPlane) {
    if bitplane >= 292 { (Err(ht_reader_error(crate::MalformedKind::InvalidCodeBlock)), plane) }
    else {
        match tier1_raw_plane_index(plane, x, y) {
            Err(error) => (Err(error), plane),
            Ok(index) => {
                let coefficient = plane.coefficients[index as int];
                let magnitude = part1_magnitude_value(coefficient.limbs);
                let value = magnitude + if bit { pow2(bitplane as nat) } else { 0nat };
                if !coefficient.significant {
                    (Err(ht_reader_error(crate::MalformedKind::InvalidCodeBlock)), plane)
                } else if value >= part1_magnitude_capacity() {
                    (Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow)), plane)
                } else {
                    (Ok(()), Tier1RawPlane { coefficients: plane.coefficients.update(index as int,
                        Tier1RawCoefficient {
                            limbs: if bit { tier1_value_limbs(value) } else { coefficient.limbs },
                            refined: true,
                            legacy_bitplane: if coefficient.legacy_bitplane.is_some() { Some(bitplane) } else { None },
                            ..coefficient
                        }), ..plane })
                }
            },
        }
    }
}

pub struct Part1LegacyFrame {
    pub plane: Tier1RawPlane,
    pub state: Part1LegacyBitplane,
    pub symbols: Part1LegacySymbols,
    pub statistics: crate::tier1::Tier1PassStatistics,
}

pub type Part1LegacyEffect = (Result<(), crate::DecodeError>, Part1LegacyFrame);
pub type Part1LegacyPassEffect = (Result<crate::tier1::Tier1PassStatistics, crate::DecodeError>, Tier1RawPlane, Part1LegacyBitplane, Part1LegacySymbols);

pub open spec fn part1_legacy_stat(s: Part1LegacyFrame, field: u8, count: u32) -> Part1LegacyEffect {
    let next = tier1_statistics_increment(s.statistics, field, count);
    (next.0, Part1LegacyFrame { statistics: next.1, ..s })
}

pub open spec fn part1_legacy_stat_then(s: Part1LegacyEffect, field: u8, count: u32) -> Part1LegacyEffect {
    match s.0 { Err(_) => s, Ok(()) => part1_legacy_stat(s.1, field, count) }
}

#[verifier::opaque]
pub open spec fn part1_legacy_sign_mark(
    plane: Tier1RawPlane, symbols: Part1LegacySymbols, x: u16, y: u16, bitplane: u16,
    summary: crate::tier1::NeighborSummary,
) -> (Result<(), crate::DecodeError>, Tier1RawPlane, Part1LegacySymbols) {
    let sign = part1_legacy_sign_context(summary);
    let decoded = part1_legacy_symbol(symbols, sign.context as usize);
    match decoded.0 {
        Err(error) => (Err(error), plane, decoded.1),
        Ok(bit) => {
            let marked = part1_legacy_mark(plane, x, y, bitplane,
                part1_tier1_decoded_sign(bit, sign.predicted_negative, symbols is Raw));
            (marked.0, marked.1, decoded.1)
        },
    }
}

pub open spec fn part1_legacy_sign_mark_stat(
    s: Part1LegacyFrame, x: u16, y: u16, summary: crate::tier1::NeighborSummary,
) -> Part1LegacyEffect {
    let marked = part1_legacy_sign_mark(s.plane, s.symbols, x, y, s.state.bitplane, summary);
    let next = Part1LegacyFrame { plane: marked.1, symbols: marked.2, ..s };
    match marked.0 {
        Err(error) => (Err(error), next),
        Ok(()) => part1_legacy_stat_then(part1_legacy_stat(next, 1, 1), 2, 1),
    }
}

#[verifier::opaque]
pub open spec fn part1_legacy_sig_cell(
    s: Part1LegacyFrame, x: u16, y: u16, orientation: crate::tier1::SubbandOrientation, causal: bool,
) -> Part1LegacyEffect {
    let scanned = part1_legacy_stat(s, 0, 1);
    match scanned.0 {
        Err(_) => scanned,
        Ok(()) => {
            let s = scanned.1;
            match tier1_raw_plane_index(s.plane, x, y) {
                Err(error) => (Err(error), s),
                Ok(index) => {
                    let coefficient = s.plane.coefficients[index as int];
                    if coefficient.significant { (Ok(()), s) } else {
                        match ht_neighbors(s.plane, x, y, causal) {
                            Err(error) => (Err(error), s),
                            Ok(summary) => {
                                if ht_significant_neighbor_count(summary) == 0 { (Ok(()), s) }
                                else if index >= s.state.significance_visited.len() {
                                    (Err(ht_reader_error(crate::MalformedKind::InvalidCodeBlock)), s)
                                } else {
                                    let bit = part1_legacy_symbol(s.symbols, part1_legacy_zero_context(summary, orientation) as usize);
                                    let decoded = Part1LegacyFrame { symbols: bit.1, ..s };
                                    match bit.0 {
                                        Err(error) => (Err(error), decoded),
                                        Ok(significant) => {
                                            let visited = Part1LegacyFrame { state: Part1LegacyBitplane {
                                                significance_visited: decoded.state.significance_visited.update(index as int, true), ..decoded.state
                                            }, ..decoded };
                                            let counted = part1_legacy_stat(visited, 1, 1);
                                            match counted.0 {
                                                Err(_) => counted,
                                                Ok(()) => if significant { part1_legacy_sign_mark_stat(counted.1, x, y, summary) } else { counted },
                                            }
                                        },
                                    }
                                }
                            },
                        }
                    }
                },
            }
        },
    }
}

#[verifier::opaque]
pub open spec fn part1_legacy_mag_cell(s: Part1LegacyFrame, x: u16, y: u16, causal: bool) -> Part1LegacyEffect {
    let scanned = part1_legacy_stat(s, 0, 1);
    match scanned.0 {
        Err(_) => scanned,
        Ok(()) => {
            let s = scanned.1;
            match tier1_raw_plane_index(s.plane, x, y) {
                Err(error) => (Err(error), s),
                Ok(index) => {
                    if index >= s.state.significant_before.len() {
                        (Err(ht_reader_error(crate::MalformedKind::InvalidCodeBlock)), s)
                    } else if !s.state.significant_before[index as int] { (Ok(()), s) }
                    else {
                        let coefficient = s.plane.coefficients[index as int];
                        match ht_neighbors(s.plane, x, y, causal) {
                            Err(error) => (Err(error), s),
                            Ok(summary) => {
                                let bit = part1_legacy_symbol(s.symbols, part1_legacy_refinement_context(coefficient, summary) as usize);
                                let decoded = Part1LegacyFrame { symbols: bit.1, ..s };
                                match bit.0 {
                                    Err(error) => (Err(error), decoded),
                                    Ok(refinement) => {
                                        let counted = part1_legacy_stat(decoded, 1, 1);
                                        match counted.0 {
                                            Err(_) => counted,
                                            Ok(()) => {
                                                let s = counted.1;
                                                let refined = part1_legacy_refine(s.plane, x, y, s.state.bitplane, refinement);
                                                let next = Part1LegacyFrame { plane: refined.1, ..s };
                                                match refined.0 {
                                                    Err(error) => (Err(error), next),
                                                    Ok(()) => part1_legacy_stat(next, 3, 1),
                                                }
                                            },
                                        }
                                    },
                                }
                            },
                        }
                    }
                },
            }
        },
    }
}

pub open spec fn part1_legacy_cell(
    s: Part1LegacyFrame, x: u16, y: u16, orientation: crate::tier1::SubbandOrientation, causal: bool, kind: u8,
) -> Part1LegacyEffect {
    if kind == 0 { part1_legacy_sig_cell(s, x, y, orientation, causal) }
    else if kind == 1 { part1_legacy_mag_cell(s, x, y, causal) }
    else {
        let scanned = part1_legacy_stat(s, 0, 1);
        match scanned.0 {
            Err(_) => scanned,
            Ok(()) => part1_legacy_cleanup_cell(scanned.1, x, y, orientation, causal),
        }
    }
}

#[verifier::opaque]
pub open spec fn part1_legacy_cleanup_cell(
    s: Part1LegacyFrame, x: u16, y: u16, orientation: crate::tier1::SubbandOrientation, causal: bool,
) -> Part1LegacyEffect {
    match tier1_raw_plane_index(s.plane, x, y) {
        Err(error) => (Err(error), s),
        Ok(index) => {
            if index >= s.state.significance_visited.len() {
                (Err(ht_reader_error(crate::MalformedKind::InvalidCodeBlock)), s)
            } else if s.plane.coefficients[index as int].significant || s.state.significance_visited[index as int] {
                (Ok(()), s)
            } else {
                match ht_neighbors(s.plane, x, y, causal) {
                    Err(error) => (Err(error), s),
                    Ok(summary) => {
                        let bit = part1_legacy_symbol(s.symbols, part1_legacy_zero_context(summary, orientation) as usize);
                        let decoded = Part1LegacyFrame { symbols: bit.1, ..s };
                        match bit.0 {
                            Err(error) => (Err(error), decoded),
                            Ok(significant) => {
                                let counted = part1_legacy_stat(decoded, 1, 1);
                                match counted.0 {
                                    Err(_) => counted,
                                    Ok(()) => if significant { part1_legacy_sign_mark_stat(counted.1, x, y, summary) } else { counted },
                                }
                            },
                        }
                    },
                }
            }
        },
    }
}

#[verifier::opaque]
pub open spec fn part1_legacy_cleanup_candidate(
    plane: Tier1RawPlane, state: Part1LegacyBitplane, x: u16, y: u16, causal: bool,
) -> Result<bool, crate::DecodeError> {
    match tier1_raw_plane_index(plane, x, y) {
        Err(error) => Err(error),
        Ok(index) => if index >= state.significance_visited.len() {
            Err(ht_reader_error(crate::MalformedKind::InvalidCodeBlock))
        } else if plane.coefficients[index as int].significant || state.significance_visited[index as int] {
            Ok(false)
        } else {
            match ht_neighbors(plane, x, y, causal) {
                Err(error) => Err(error),
                Ok(summary) => Ok(ht_significant_neighbor_count(summary) == 0),
            }
        },
    }
}

#[verifier::opaque]
pub open spec fn part1_legacy_cleanup_run(
    plane: Tier1RawPlane, state: Part1LegacyBitplane, x: u16, stripe: u16, rows: u16, causal: bool,
) -> Result<bool, crate::DecodeError> {
    if stripe as nat + 3 > u16::MAX { Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow)) }
    else if rows != 4 { Ok(false) }
    else {
        match part1_legacy_cleanup_candidate(plane, state, x, stripe, causal) {
            Err(error) => Err(error),
            Ok(false) => Ok(false),
            Ok(true) => match part1_legacy_cleanup_candidate(plane, state, x, (stripe + 1) as u16, causal) {
                Err(error) => Err(error),
                Ok(false) => Ok(false),
                Ok(true) => match part1_legacy_cleanup_candidate(plane, state, x, (stripe + 2) as u16, causal) {
                    Err(error) => Err(error),
                    Ok(false) => Ok(false),
                    Ok(true) => part1_legacy_cleanup_candidate(plane, state, x, (stripe + 3) as u16, causal),
                },
            },
        }
    }
}

pub open spec fn part1_legacy_cleanup_rows(
    s: Part1LegacyFrame, x: u16, stripe: u16, row: nat, rows: nat, orientation: crate::tier1::SubbandOrientation, causal: bool,
) -> Part1LegacyEffect
    decreases rows - row,
{
    if row >= rows { (Ok(()), s) }
    else if stripe as nat + row > u16::MAX { (Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow)), s) }
    else {
        let step = part1_legacy_cleanup_cell(s, x, (stripe as nat + row) as u16, orientation, causal);
        match step.0 {
            Err(_) => step,
            Ok(()) => part1_legacy_cleanup_rows(step.1, x, stripe, row + 1, rows, orientation, causal),
        }
    }
}

#[verifier::opaque]
pub open spec fn part1_legacy_cleanup_interrupt(
    s: Part1LegacyFrame, x: u16, stripe: u16, orientation: crate::tier1::SubbandOrientation, causal: bool,
) -> Part1LegacyEffect {
    let high = part1_legacy_symbol(s.symbols, 18);
    let s = Part1LegacyFrame { symbols: high.1, ..s };
    match high.0 {
        Err(error) => (Err(error), s),
        Ok(high) => {
            let low = part1_legacy_symbol(s.symbols, 18);
            let s = Part1LegacyFrame { symbols: low.1, ..s };
            match low.0 {
                Err(error) => (Err(error), s),
                Ok(low) => {
                    let counted = part1_legacy_stat(s, 1, 2);
                    match counted.0 {
                        Err(_) => counted,
                        Ok(()) => {
                            let s = counted.1;
                            let run: nat = if high { if low { 3nat } else { 2nat } } else if low { 1nat } else { 0nat };
                            if stripe as nat + run > u16::MAX { (Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow)), s) }
                            else {
                                let y = (stripe as nat + run) as u16;
                                match ht_neighbors(s.plane, x, y, causal) {
                                    Err(error) => (Err(error), s),
                                    Ok(summary) => {
                                        let marked = part1_legacy_sign_mark_stat(s, x, y, summary);
                                        match marked.0 {
                                            Err(_) => marked,
                                            Ok(()) => part1_legacy_cleanup_rows(marked.1, x, stripe, run + 1, 4, orientation, causal),
                                        }
                                    },
                                }
                            }
                        },
                    }
                },
            }
        },
    }
}

#[verifier::opaque]
pub open spec fn part1_legacy_cleanup_column(
    s: Part1LegacyFrame, x: u16, stripe: u16, rows: u16, orientation: crate::tier1::SubbandOrientation, causal: bool,
) -> Part1LegacyEffect {
    match part1_legacy_cleanup_run(s.plane, s.state, x, stripe, rows, causal) {
        Err(error) => (Err(error), s),
        Ok(false) => part1_legacy_rows(s, x, stripe, 0, rows as nat, orientation, causal, 2),
        Ok(true) => {
            let counted = part1_legacy_stat_then(part1_legacy_stat(s, 0, 4), 4, 1);
            match counted.0 {
                Err(_) => counted,
                Ok(()) => {
                    let s = counted.1;
                    let interrupted = part1_legacy_symbol(s.symbols, 17);
                    let s = Part1LegacyFrame { symbols: interrupted.1, ..s };
                    match interrupted.0 {
                        Err(error) => (Err(error), s),
                        Ok(interrupted) => {
                            let counted = part1_legacy_stat(s, 1, 1);
                            match counted.0 {
                                Err(_) => counted,
                                Ok(()) => if interrupted { part1_legacy_cleanup_interrupt(counted.1, x, stripe, orientation, causal) } else { counted },
                            }
                        },
                    }
                },
            }
        },
    }
}

pub open spec fn part1_legacy_rows(
    s: Part1LegacyFrame, x: u16, stripe: u16, row: nat, rows: nat, orientation: crate::tier1::SubbandOrientation, causal: bool, kind: u8,
) -> Part1LegacyEffect
    decreases rows - row,
{
    if row >= rows { (Ok(()), s) }
    else if stripe as nat + row > u16::MAX {
        (Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow)), s)
    } else {
        let step = part1_legacy_cell(s, x, (stripe as nat + row) as u16, orientation, causal, kind);
        match step.0 {
            Err(_) => step,
            Ok(()) => part1_legacy_rows(step.1, x, stripe, row + 1, rows, orientation, causal, kind),
        }
    }
}

pub open spec fn part1_legacy_columns(
    s: Part1LegacyFrame, x: nat, stripe: u16, rows: nat, width: u16, orientation: crate::tier1::SubbandOrientation, causal: bool, kind: u8,
) -> Part1LegacyEffect
    decreases width - x, 0nat,
{
    if x >= width { (Ok(()), s) } else {
        let step = if kind == 2 {
            part1_legacy_cleanup_column(s, x as u16, stripe, rows as u16, orientation, causal)
        } else { part1_legacy_rows(s, x as u16, stripe, 0, rows, orientation, causal, kind) };
        part1_legacy_after_rows(step, x + 1, stripe, rows, width, orientation, causal, kind)
    }
}

pub open spec fn part1_legacy_after_rows(
    step: Part1LegacyEffect, x: nat, stripe: u16, rows: nat, width: u16, orientation: crate::tier1::SubbandOrientation, causal: bool, kind: u8,
) -> Part1LegacyEffect
    decreases width - x, 1nat,
{
    match step.0 {
        Err(_) => step,
        Ok(()) => part1_legacy_columns(step.1, x, stripe, rows, width, orientation, causal, kind),
    }
}

pub open spec fn part1_legacy_stripes(
    s: Part1LegacyFrame, stripe: nat, width: u16, height: u16, orientation: crate::tier1::SubbandOrientation, causal: bool, kind: u8,
) -> Part1LegacyEffect
    decreases height - stripe, 0nat,
{
    if stripe >= height { (Ok(()), s) } else {
        let rows = if height - stripe < 4 { (height - stripe) as nat } else { 4nat };
        let step = part1_legacy_columns(s, 0, stripe as u16, rows, width, orientation, causal, kind);
        part1_legacy_after_columns(step, stripe + rows, width, height, orientation, causal, kind)
    }
}

pub open spec fn part1_legacy_after_columns(
    step: Part1LegacyEffect, stripe: nat, width: u16, height: u16, orientation: crate::tier1::SubbandOrientation, causal: bool, kind: u8,
) -> Part1LegacyEffect
    decreases height - stripe, 1nat,
{
    match step.0 {
        Err(_) => step,
        Ok(()) => part1_legacy_stripes(step.1, stripe, width, height, orientation, causal, kind),
    }
}

pub open spec fn part1_legacy_pass(
    plane: Tier1RawPlane, state: Part1LegacyBitplane, symbols: Part1LegacySymbols,
    orientation: crate::tier1::SubbandOrientation, causal: bool, kind: u8,
) -> Part1LegacyPassEffect {
    let validation = part1_legacy_validate(state, plane);
    let order_valid = if kind == 0 {
        !state.significance_propagation_complete && !state.magnitude_refinement_complete && !state.cleanup_complete
    } else {
        state.significance_propagation_complete && !state.magnitude_refinement_complete && !state.cleanup_complete
    };
    match validation {
        Err(error) => (Err(error), plane, state, symbols),
        Ok(()) => if !order_valid {
            (Err(ht_reader_error(crate::MalformedKind::InvalidCodeBlock)), plane, state, symbols)
        } else {
            let run = part1_legacy_stripes(Part1LegacyFrame { plane, state, symbols, statistics: tier1_statistics_zero() },
                0, plane.width, plane.height, orientation, causal, kind);
            part1_legacy_finish(run, kind)
        },
    }
}

pub open spec fn part1_legacy_finish(run: Part1LegacyEffect, kind: u8) -> Part1LegacyPassEffect {
    let next = run.1;
    match run.0 {
        Err(error) => (Err(error), next.plane, next.state, next.symbols),
        Ok(()) => (Ok(next.statistics), next.plane, if kind == 0 {
            Part1LegacyBitplane { significance_propagation_complete: true, ..next.state }
        } else {
            Part1LegacyBitplane { magnitude_refinement_complete: true, ..next.state }
        }, next.symbols),
    }
}

pub open spec fn part1_legacy_column_tail(
    s: Part1LegacyFrame, x: nat, stripe: u16, rows: nat, width: u16, height: u16,
    orientation: crate::tier1::SubbandOrientation, causal: bool, kind: u8,
) -> Part1LegacyPassEffect {
    part1_legacy_finish(part1_legacy_after_columns(
        part1_legacy_columns(s, x, stripe, rows, width, orientation, causal, kind),
        stripe as nat + rows, width, height, orientation, causal, kind), kind)
}

pub open spec fn part1_legacy_row_tail(
    s: Part1LegacyFrame, x: u16, stripe: u16, row: nat, rows: nat, width: u16, height: u16,
    orientation: crate::tier1::SubbandOrientation, causal: bool, kind: u8,
) -> Part1LegacyPassEffect {
    part1_legacy_finish(part1_legacy_after_columns(part1_legacy_after_rows(
        part1_legacy_rows(s, x, stripe, row, rows, orientation, causal, kind),
        x as nat + 1, stripe, rows, width, orientation, causal, kind),
        stripe as nat + rows, width, height, orientation, causal, kind), kind)
}

#[verifier::opaque]
pub open spec fn part1_legacy_segsym(s: Part1LegacyFrame) -> Part1LegacyEffect {
    let a = part1_legacy_symbol(s.symbols, 18);
    let s = Part1LegacyFrame { symbols: a.1, ..s };
    match a.0 {
        Err(error) => (Err(error), s),
        Ok(a) => {
            let b = part1_legacy_symbol(s.symbols, 18);
            let s = Part1LegacyFrame { symbols: b.1, ..s };
            match b.0 {
                Err(error) => (Err(error), s),
                Ok(b) => {
                    let c = part1_legacy_symbol(s.symbols, 18);
                    let s = Part1LegacyFrame { symbols: c.1, ..s };
                    match c.0 {
                        Err(error) => (Err(error), s),
                        Ok(c) => {
                            let d = part1_legacy_symbol(s.symbols, 18);
                            let s = Part1LegacyFrame { symbols: d.1, ..s };
                            match d.0 {
                                Err(error) => (Err(error), s),
                                Ok(d) => {
                                    let counted = part1_legacy_stat(s, 1, 4);
                                    match counted.0 {
                                        Err(_) => counted,
                                        Ok(()) => if !a || b || !c || d {
                                            (Err(ht_reader_error(crate::MalformedKind::InvalidCodeBlock)), counted.1)
                                        } else { counted },
                                    }
                                },
                            }
                        },
                    }
                },
            }
        },
    }
}

pub open spec fn part1_legacy_cleanup_finish(run: Part1LegacyEffect, segsym: bool) -> Part1LegacyPassEffect {
    let after = match run.0 {
        Err(_) => run,
        Ok(()) => if segsym { part1_legacy_segsym(run.1) } else { run },
    };
    let next = after.1;
    match after.0 {
        Err(error) => (Err(error), next.plane, next.state, next.symbols),
        Ok(()) => (Ok(next.statistics), next.plane, Part1LegacyBitplane { cleanup_complete: true, ..next.state }, next.symbols),
    }
}

pub open spec fn part1_legacy_cleanup_pass(
    plane: Tier1RawPlane, state: Part1LegacyBitplane, symbols: Part1LegacySymbols,
    orientation: crate::tier1::SubbandOrientation, causal: bool, segsym: bool,
) -> Part1LegacyPassEffect {
    match part1_legacy_validate(state, plane) {
        Err(error) => (Err(error), plane, state, symbols),
        Ok(()) => if state.cleanup_complete || state.significance_propagation_complete != state.magnitude_refinement_complete {
            (Err(ht_reader_error(crate::MalformedKind::InvalidCodeBlock)), plane, state, symbols)
        } else {
            part1_legacy_cleanup_finish(part1_legacy_stripes(
                Part1LegacyFrame { plane, state, symbols, statistics: tier1_statistics_zero() },
                0, plane.width, plane.height, orientation, causal, 2), segsym)
        },
    }
}

pub open spec fn part1_legacy_cleanup_column_tail(
    s: Part1LegacyFrame, x: nat, stripe: u16, rows: nat, width: u16, height: u16,
    orientation: crate::tier1::SubbandOrientation, causal: bool, segsym: bool,
) -> Part1LegacyPassEffect {
    part1_legacy_cleanup_finish(part1_legacy_after_columns(
        part1_legacy_columns(s, x, stripe, rows, width, orientation, causal, 2),
        stripe as nat + rows, width, height, orientation, causal, 2), segsym)
}

}
