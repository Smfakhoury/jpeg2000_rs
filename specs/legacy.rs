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

pub open spec fn part1_legacy_pass_kind(index: u32) -> crate::tier1::Tier1PassKind {
    let code = part1_tier1_pass_code(index as nat);
    if code == 0 { crate::tier1::Tier1PassKind::SignificancePropagation }
    else if code == 1 { crate::tier1::Tier1PassKind::MagnitudeRefinement }
    else { crate::tier1::Tier1PassKind::Cleanup }
}

pub open spec fn part1_legacy_uses_raw(style: crate::tier1::Tier1CodingStyle, index: u32) -> bool {
    style.selective_arithmetic_coding_bypass && index >= 10 && !(part1_legacy_pass_kind(index) is Cleanup)
}

pub open spec fn part1_legacy_plan(style: crate::tier1::Tier1CodingStyle, index: u32) -> crate::tier1::Tier1PassPlan {
    let kind = part1_legacy_pass_kind(index);
    let raw = part1_legacy_uses_raw(style, index);
    let terminated = style.terminate_all_passes || (style.selective_arithmetic_coding_bypass
        && (index == 9 || (index >= 10 && !(kind is SignificancePropagation))));
    crate::tier1::Tier1PassPlan {
        kind, coding_mode: if raw { crate::tier1::Tier1CodingMode::Raw } else { crate::tier1::Tier1CodingMode::Mq },
        terminates_segment: terminated, reset_contexts_after_pass: style.reset_contexts,
        validate_predictable_termination: style.predictable_termination && terminated && !raw,
        expect_segmentation_symbol: style.segmentation_symbols && (kind is Cleanup),
    }
}

pub open spec fn part1_legacy_segment_passes(style: crate::tier1::Tier1CodingStyle, first: u32, available: u32) -> u32 {
    if available == 0 { 0 } else if style.terminate_all_passes { 1 }
    else if !style.selective_arithmetic_coding_bypass { available }
    else if first < 10 { if available < 10 - first { available } else { (10 - first) as u32 } }
    else if part1_legacy_pass_kind(first) is SignificancePropagation {
        if available < 2 { available } else { 2 }
    } else { 1 }
}

pub open spec fn part1_legacy_ensure_bitplane(s: Tier1DecoderState) -> (Result<(), crate::DecodeError>, Tier1DecoderState) {
    if s.bitplane_state.is_some() { (Ok(()), s) } else {
        match part1_legacy_bitplane_initial(s.plane, s.bitplane) {
            Err(error) => (Err(error), s),
            Ok(state) => (Ok(()), Tier1DecoderState { bitplane_state: Some(state), ..s }),
        }
    }
}

pub open spec fn part1_legacy_advance(s: Tier1DecoderState, kind: crate::tier1::Tier1PassKind) -> (Result<(), crate::DecodeError>, Tier1DecoderState) {
    if s.next_pass == u32::MAX { (Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow)), s) }
    else {
        let next = Tier1DecoderState { next_pass: (s.next_pass + 1) as u32, ..s };
        if kind is Cleanup {
            let next = Tier1DecoderState { bitplane_state: None, ..next };
            if next.next_pass < next.maximum_passes {
                if next.bitplane == 0 { (Err(ht_reader_error(crate::MalformedKind::InvalidCodeBlock)), next) }
                else { (Ok(()), Tier1DecoderState { bitplane: (next.bitplane - 1) as u16, ..next }) }
            } else { (Ok(()), next) }
        } else { (Ok(()), next) }
    }
}

#[verifier::opaque]
pub open spec fn part1_legacy_decode_pass(s: Tier1DecoderState, symbols: Part1LegacySymbols, kind: crate::tier1::Tier1PassKind)
    -> (Result<crate::tier1::Tier1PassStatistics, crate::DecodeError>, Tier1DecoderState, Part1LegacySymbols)
{
    let ready = part1_legacy_ensure_bitplane(s);
    match ready.0 {
        Err(error) => (Err(error), ready.1, symbols),
        Ok(()) => {
            let s = ready.1;
            match s.bitplane_state {
                None => (Err(ht_reader_error(crate::MalformedKind::InvalidCodeBlock)), s, symbols),
                Some(state) => {
                    if (symbols is Raw) && (kind is Cleanup) {
                        (Err(ht_reader_error(crate::MalformedKind::InvalidCodeBlock)), s, symbols)
                    } else {
                        let pass = if kind is Cleanup {
                            part1_legacy_cleanup_pass(s.plane, state, symbols, s.orientation, s.style.vertical_causal_context, s.style.segmentation_symbols)
                        } else {
                            part1_legacy_pass(s.plane, state, symbols,
                                if kind is MagnitudeRefinement { crate::tier1::SubbandOrientation::Ll } else { s.orientation },
                                s.style.vertical_causal_context, if kind is MagnitudeRefinement { 1u8 } else { 0u8 })
                        };
                        (pass.0, Tier1DecoderState { plane: pass.1, bitplane_state: Some(pass.2), ..s }, pass.3)
                    }
                },
            }
        },
    }
}

pub struct Part1LegacySegmentRun {
    pub decoder: Tier1DecoderState,
    pub symbols: Part1LegacySymbols,
    pub statistics: crate::tier1::Tier1PassStatistics,
}

pub type Part1LegacySegmentEffect = (Result<(), crate::DecodeError>, Part1LegacySegmentRun);
pub type Part1LegacySegmentPassEffect = (Result<crate::tier1::Tier1PassStatistics, crate::DecodeError>, Tier1DecoderState, Part1LegacySymbols);

#[verifier::opaque]
pub open spec fn part1_legacy_segment_step(s: Part1LegacySegmentRun) -> Part1LegacySegmentEffect {
    let plan = part1_legacy_plan(s.decoder.style, s.decoder.next_pass);
    if (plan.coding_mode is Raw) != (s.symbols is Raw) {
        (Err(ht_reader_error(crate::MalformedKind::InvalidCodeBlock)), s)
    } else {
        let decoded = part1_legacy_decode_pass(s.decoder, s.symbols, plan.kind);
        let s = Part1LegacySegmentRun { decoder: decoded.1, symbols: decoded.2, ..s };
        match decoded.0 {
            Err(error) => (Err(error), s),
            Ok(statistics) => {
                let merged = tier1_statistics_merge(s.statistics, statistics);
                let s = Part1LegacySegmentRun { statistics: merged.1, ..s };
                match merged.0 {
                    Err(error) => (Err(error), s),
                    Ok(()) => {
                        let advanced = part1_legacy_advance(s.decoder, plan.kind);
                        let s = Part1LegacySegmentRun { decoder: advanced.1, ..s };
                        match advanced.0 {
                            Err(error) => (Err(error), s),
                            Ok(()) => {
                                if s.symbols is Raw {
                                    (Ok(()), Part1LegacySegmentRun { decoder: if s.decoder.style.reset_contexts {
                                        Tier1DecoderState { reset_contexts_before_mq: true, ..s.decoder }
                                    } else { s.decoder }, ..s })
                                } else {
                                    let mq = s.symbols->Mq_0;
                                    (Ok(()), Part1LegacySegmentRun { symbols: Part1LegacySymbols::Mq(
                                        if s.decoder.style.reset_contexts { part1_legacy_initialized(mq) } else { mq }), ..s })
                                }
                            },
                        }
                    },
                }
            },
        }
    }
}

pub open spec fn part1_legacy_segment_run(s: Part1LegacySegmentRun, remaining: nat) -> Part1LegacySegmentEffect
    decreases remaining,
{
    if remaining == 0 { (Ok(()), s) } else {
        let step = part1_legacy_segment_step(s);
        match step.0 {
            Err(_) => step,
            Ok(()) => part1_legacy_segment_run(step.1, (remaining - 1) as nat),
        }
    }
}

pub open spec fn part1_legacy_segment_finish(run: Part1LegacySegmentEffect) -> Part1LegacySegmentPassEffect {
    let s = run.1;
    (match run.0 { Err(error) => Err(error), Ok(()) => Ok(s.statistics) }, s.decoder, s.symbols)
}

#[verifier::opaque]
pub open spec fn part1_legacy_segment_admission(
    s: Tier1DecoderState, input: Seq<u8>, count: u32, pass_limit: u32, byte_limit: u64,
) -> Result<(), crate::DecodeError> {
    if count == 0 || (input.len() == 0 && s.next_pass == 0) { Err(ht_reader_error(crate::MalformedKind::InvalidCodeBlock)) }
    else if s.next_pass as nat + count as nat > u32::MAX { Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow)) }
    else {
        let next = (s.next_pass + count) as u32;
        if next > pass_limit {
            Err(part1_mq_limit(crate::Resource::CodeBlockPasses, pass_limit as u64, next as u64))
        } else if s.decoded_bytes as nat + input.len() > u64::MAX {
            Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow))
        } else {
            let bytes = (s.decoded_bytes as nat + input.len()) as u64;
            if bytes > byte_limit { Err(part1_mq_limit(crate::Resource::CodeBlockBytes, byte_limit, bytes)) }
            else if next > s.maximum_passes || part1_legacy_segment_passes(s.style, s.next_pass, count) != count {
                Err(ht_reader_error(crate::MalformedKind::InvalidCodeBlock))
            } else if input.len() == 0 && next != s.maximum_passes && !part1_legacy_plan(s.style, (next - 1) as u32).terminates_segment {
                Err(ht_reader_error(crate::MalformedKind::InvalidCodeBlock))
            } else { Ok(()) }
        }
    }
}

pub open spec fn part1_legacy_segment_setup(s: Tier1DecoderState, mq: Part1MqState) -> (Tier1DecoderState, Part1MqState) {
    if s.reset_contexts_before_mq || s.mq_context_state.is_none() {
        (Tier1DecoderState { reset_contexts_before_mq: false, ..s }, part1_legacy_initialized(mq))
    } else {
        match s.mq_context_state {
            None => (s, mq),
            Some(contexts) => (s, Part1MqState { contexts, ..mq }),
        }
    }
}

#[verifier::opaque]
pub open spec fn part1_legacy_segment_account(
    s: Tier1DecoderState, bytes: nat, count: u32, statistics: crate::tier1::Tier1PassStatistics,
) -> (Result<crate::tier1::Tier1SegmentDecode, crate::DecodeError>, Tier1DecoderState) {
    if s.decoded_segments == u32::MAX { (Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow)), s) }
    else {
        let s = Tier1DecoderState { decoded_segments: (s.decoded_segments + 1) as u32, ..s };
        if s.decoded_bytes as nat + bytes > u64::MAX { (Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow)), s) }
        else {
            let s = Tier1DecoderState { decoded_bytes: (s.decoded_bytes as nat + bytes) as u64, ..s };
            if bytes > u32::MAX { (Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow)), s) }
            else { (Ok(crate::tier1::Tier1SegmentDecode { passes: count, bytes: bytes as u32, statistics }), s) }
        }
    }
}

pub open spec fn part1_legacy_segment_mq_finish(
    run: Part1LegacySegmentPassEffect, bytes: nat, count: u32,
) -> (Result<crate::tier1::Tier1SegmentDecode, crate::DecodeError>, Tier1DecoderState) {
    let s = run.1;
    match run.0 {
        Err(error) => (Err(error), s),
        Ok(statistics) => match run.2 {
            Part1LegacySymbols::Raw(_) => (Err(ht_reader_error(crate::MalformedKind::InvalidMqState)), s),
            Part1LegacySymbols::Mq(mq) => {
                let term = if s.style.predictable_termination { part1_mq_termination(mq) } else { Ok(()) };
                match term {
                    Err(error) => (Err(error), s),
                    Ok(()) => part1_legacy_segment_account(
                        Tier1DecoderState { mq_context_state: Some(mq.contexts), ..s }, bytes, count, statistics),
                }
            },
        },
    }
}

pub open spec fn part1_legacy_segment_raw_finish(
    run: Part1LegacySegmentPassEffect, bytes: nat, count: u32,
) -> (Result<crate::tier1::Tier1SegmentDecode, crate::DecodeError>, Tier1DecoderState) {
    let s = run.1;
    match run.0 {
        Err(error) => (Err(error), s),
        Ok(statistics) => match run.2 {
            Part1LegacySymbols::Mq(_) => (Err(ht_reader_error(crate::MalformedKind::InvalidMqState)), s),
            Part1LegacySymbols::Raw(raw) => {
                let term = if s.style.predictable_termination { part1_raw_reader_termination(raw) } else { Ok(()) };
                match term {
                    Err(error) => (Err(error), s),
                    Ok(()) => part1_legacy_segment_account(s, bytes, count, statistics),
                }
            },
        },
    }
}

#[verifier::opaque]
pub open spec fn part1_legacy_segment(
    s: Tier1DecoderState, input: Seq<u8>, count: u32, pass_limit: u32, byte_limit: u64, input_limit: u64, work_limit: u64,
) -> (Result<crate::tier1::Tier1SegmentDecode, crate::DecodeError>, Tier1DecoderState) {
    match part1_legacy_segment_admission(s, input, count, pass_limit, byte_limit) {
        Err(error) => (Err(error), s),
        Ok(()) => {
            if part1_legacy_plan(s.style, s.next_pass).coding_mode is Mq {
                match part1_mq_initial(input, input_limit, work_limit) {
                    Err(error) => (Err(error), s),
                    Ok(mq) => {
                        let ready = part1_legacy_segment_setup(s, mq);
                        part1_legacy_segment_mq_finish(part1_legacy_segment_finish(part1_legacy_segment_run(
                            Part1LegacySegmentRun { decoder: ready.0, symbols: Part1LegacySymbols::Mq(ready.1), statistics: tier1_statistics_zero() },
                            count as nat)), input.len(), count)
                    },
                }
            } else {
                match part1_raw_reader_initial(input, input_limit, work_limit) {
                    Err(error) => (Err(error), s),
                    Ok(raw) => part1_legacy_segment_raw_finish(part1_legacy_segment_finish(part1_legacy_segment_run(
                        Part1LegacySegmentRun { decoder: s, symbols: Part1LegacySymbols::Raw(raw), statistics: tier1_statistics_zero() },
                        count as nat)), input.len(), count),
                }
            }
        },
    }
}

#[verifier::opaque]
pub open spec fn part1_legacy_contribution_work(
    samples: nat, replay: u32, bytes: u64, previous: u64, maximum: u64,
) -> Result<u64, crate::DecodeError> {
    let base = samples * replay as nat;
    if base > u64::MAX || base + bytes as nat > u64::MAX || base + bytes as nat + previous as nat > u64::MAX {
        Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow))
    } else {
        let work = (base + bytes as nat + previous as nat) as u64;
        if work > maximum { Err(part1_mq_limit(crate::Resource::WorkUnits, maximum, work)) } else { Ok(work) }
    }
}

#[verifier::opaque]
pub open spec fn part1_legacy_contribution_admission(
    s: Tier1DecoderState, input: Seq<u8>, passes: u8, byte_limit: u64, work_limit: u64,
) -> (Result<(u32, u32, u32), crate::DecodeError>, Tier1DecoderState) {
    if passes == 0 { (Err(ht_reader_error(crate::MalformedKind::InvalidCodeBlock)), s) }
    else if s.decoded_bytes as nat + input.len() > u64::MAX { (Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow)), s) }
    else {
        let bytes = (s.decoded_bytes as nat + input.len()) as u64;
        if bytes > byte_limit { (Err(part1_mq_limit(crate::Resource::CodeBlockBytes, byte_limit, bytes)), s) }
        else {
            let previous = match s.legacy_segment { None => 0u32, Some(segment) => segment.passes };
            if previous as nat + passes as nat > u32::MAX { (Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow)), s) }
            else {
                let replay = (previous + passes) as u32;
                match part1_legacy_contribution_work(s.plane.coefficients.len(), replay, bytes, s.legacy_decode_work, work_limit) {
                    Err(error) => (Err(error), s),
                    Ok(work) => {
                        let s = Tier1DecoderState { legacy_decode_work: work, ..s };
                        if s.next_pass as nat + passes as nat > u32::MAX {
                            (Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow)), s)
                        } else {
                            let next = (s.next_pass + passes) as u32;
                            if next == 0 { (Err(ht_reader_error(crate::MalformedKind::InvalidCodeBlock)), s) }
                            else { (Ok((replay, next, (next - 1) as u32)), s) }
                        }
                    },
                }
            }
        }
    }
}

pub open spec fn part1_legacy_snapshot(s: Tier1DecoderState) -> Tier1LegacyFrame {
    Tier1LegacyFrame {
        plane: s.plane, next_pass: s.next_pass, bitplane: s.bitplane, bitplane_state: s.bitplane_state,
        mq_context_state: s.mq_context_state, reset_contexts_before_mq: s.reset_contexts_before_mq,
        decoded_segments: s.decoded_segments, decoded_bytes: s.decoded_bytes, input: Seq::empty(), passes: 0,
    }
}

pub open spec fn part1_legacy_append(segment: Tier1LegacyFrame, input: Seq<u8>, passes: u32) -> Tier1LegacyFrame {
    Tier1LegacyFrame { input: segment.input + input, passes, ..segment }
}

pub open spec fn part1_legacy_restore(s: Tier1DecoderState, segment: Tier1LegacyFrame) -> Tier1DecoderState {
    Tier1DecoderState {
        plane: segment.plane, next_pass: segment.next_pass, bitplane: segment.bitplane, bitplane_state: segment.bitplane_state,
        mq_context_state: segment.mq_context_state, reset_contexts_before_mq: segment.reset_contexts_before_mq,
        decoded_segments: segment.decoded_segments, decoded_bytes: segment.decoded_bytes, ..s
    }
}

#[verifier::opaque]
pub open spec fn part1_legacy_replay_commit(s: Tier1DecoderState, segment: Tier1LegacyFrame)
    -> (Result<(), crate::DecodeError>, Tier1DecoderState)
{
    if s.next_pass == 0 { (Err(ht_reader_error(crate::MalformedKind::InvalidCodeBlock)), s) }
    else if s.next_pass < s.maximum_passes && !part1_legacy_plan(s.style, (s.next_pass - 1) as u32).terminates_segment {
        (Ok(()), Tier1DecoderState { legacy_segment: Some(segment), ..s })
    } else { (Ok(()), s) }
}

pub open spec fn part1_legacy_contribution_finish(
    decoded: (Result<crate::tier1::Tier1SegmentDecode, crate::DecodeError>, Tier1DecoderState),
    segment: Tier1LegacyFrame,
) -> (Result<(), crate::DecodeError>, Tier1DecoderState) {
    let s = decoded.1;
    match decoded.0 {
        Err(error) => (Err(error), s),
        Ok(_) => part1_legacy_replay_commit(s, segment),
    }
}

#[verifier::opaque]
pub open spec fn part1_legacy_replay(
    s: Tier1DecoderState, input: Seq<u8>, replay: u32, pass_limit: u32, byte_limit: u64, input_limit: u64, work_limit: u64,
) -> (Result<(), crate::DecodeError>, Tier1DecoderState) {
    let previous = s.legacy_segment;
    let s = Tier1DecoderState { legacy_segment: None, ..s };
    let segment = match previous { Some(segment) => segment, None => part1_legacy_snapshot(s) };
    let segment = part1_legacy_append(segment, input, replay);
    let restored = part1_legacy_restore(s, segment);
    part1_legacy_contribution_finish(
        part1_legacy_segment(restored, segment.input, segment.passes, pass_limit, byte_limit, input_limit, work_limit), segment)
}

#[verifier::opaque]
pub open spec fn part1_legacy_contribution(
    s: Tier1DecoderState, input: Seq<u8>, passes: u8, pass_limit: u32, byte_limit: u64, input_limit: u64, work_limit: u64,
) -> (Result<(), crate::DecodeError>, Tier1DecoderState) {
    let admitted = part1_legacy_contribution_admission(s, input, passes, byte_limit, work_limit);
    match admitted.0 {
        Err(error) => (Err(error), admitted.1),
        Ok((replay, next, last)) => {
            let s = admitted.1;
            if s.legacy_segment.is_none() && (next == s.maximum_passes || part1_legacy_plan(s.style, last).terminates_segment) {
                let decoded = part1_legacy_segment(s, input, passes as u32, pass_limit, byte_limit, input_limit, work_limit);
                (match decoded.0 { Err(error) => Err(error), Ok(_) => Ok(()) }, decoded.1)
            } else {
                part1_legacy_replay(s, input, replay, pass_limit, byte_limit, input_limit, work_limit)
            }
        },
    }
}

pub open spec fn part1_legacy_zero_bitplanes(s: Tier1DecoderState, zero: u16)
    -> (Result<(), crate::DecodeError>, Tier1DecoderState)
{
    match s.zero_bitplanes {
        Some(previous) => if previous == zero { (Ok(()), s) }
            else { (Err(ht_reader_error(crate::MalformedKind::InvalidCodeBlock)), s) },
        None => if s.next_pass != 0 || s.decoded_segments != 0 || zero > s.bitplane {
            (Err(ht_reader_error(crate::MalformedKind::InvalidCodeBlock)), s)
        } else {
            let bitplane = (s.bitplane - zero) as u16;
            let maximum = bitplane as nat * 3 + 1;
            if maximum > u32::MAX { (Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow)), s) }
            else { (Ok(()), Tier1DecoderState { bitplane, maximum_passes: maximum as u32, zero_bitplanes: Some(zero), ..s }) }
        },
    }
}

}
