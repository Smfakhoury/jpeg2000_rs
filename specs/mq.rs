use vstd::prelude::*;

verus! {

pub type Part1MqContext = (u8, bool);
pub type Part1MqProbability = (u16, u8, u8, bool);

pub struct Part1MqState {
    pub input: Seq<u8>,
    pub byte_index: usize,
    pub a: u32,
    pub c: u32,
    pub ct: u8,
    pub contexts: Seq<Part1MqContext>,
    pub work_units: u64,
    pub maximum_work_units: u64,
    pub marker_padding_count: u64,
}

pub type Part1MqEffect<T> = (Result<T, crate::DecodeError>, Part1MqState);

pub open spec fn part1_mq_probabilities() -> Seq<Part1MqProbability> {
    seq![
        (0x5601u16,1u8,1u8,true), (0x3401,2,6,false), (0x1801,3,9,false),
        (0x0ac1,4,12,false), (0x0521,5,29,false), (0x0221,38,33,false),
        (0x5601,7,6,true), (0x5401,8,14,false), (0x4801,9,14,false),
        (0x3801,10,14,false), (0x3001,11,17,false), (0x2401,12,18,false),
        (0x1c01,13,20,false), (0x1601,29,21,false), (0x5601,15,14,true),
        (0x5401,16,14,false), (0x5101,17,15,false), (0x4801,18,16,false),
        (0x3801,19,17,false), (0x3401,20,18,false), (0x3001,21,19,false),
        (0x2801,22,19,false), (0x2401,23,20,false), (0x2201,24,21,false),
        (0x1c01,25,22,false), (0x1801,26,23,false), (0x1601,27,24,false),
        (0x1401,28,25,false), (0x1201,29,26,false), (0x1101,30,27,false),
        (0x0ac1,31,28,false), (0x09c1,32,29,false), (0x08a1,33,30,false),
        (0x0521,34,31,false), (0x0441,35,32,false), (0x02a1,36,33,false),
        (0x0221,37,34,false), (0x0141,38,35,false), (0x0111,39,36,false),
        (0x0085,40,37,false), (0x0049,41,38,false), (0x0025,42,39,false),
        (0x0015,43,40,false), (0x0009,44,41,false), (0x0005,45,42,false),
        (0x0001,45,43,false), (0x5601,46,46,false)
    ]
}

pub open spec fn part1_mq_error(index: usize, kind: crate::MalformedKind) -> crate::DecodeError {
    crate::DecodeError::MalformedInput { offset: index as u64, kind }
}

pub open spec fn part1_mq_limit(resource: crate::Resource, limit: u64, actual: u64) -> crate::DecodeError {
    crate::DecodeError::LimitExceeded { resource, limit, actual }
}

pub open spec fn part1_mq_current(s: Part1MqState) -> u8 {
    if s.byte_index < s.input.len() { s.input[s.byte_index as int] } else { 0xff }
}

pub open spec fn part1_mq_next(s: Part1MqState) -> u8 {
    if s.byte_index < usize::MAX && s.byte_index + 1 < s.input.len() {
        s.input[s.byte_index as int + 1]
    } else { 0xff }
}

pub open spec fn part1_mq_advance(s: Part1MqState) -> Part1MqEffect<()> {
    if s.byte_index < s.input.len() {
        if s.byte_index == usize::MAX {
            (Err(part1_mq_error(s.byte_index, crate::MalformedKind::ArithmeticOverflow)), s)
        } else {
            (Ok(()), Part1MqState { byte_index: (s.byte_index + 1) as usize, ..s })
        }
    } else { (Ok(()), s) }
}

pub open spec fn part1_mq_charge(s: Part1MqState) -> Part1MqEffect<()> {
    if s.work_units == u64::MAX {
        (Err(part1_mq_error(s.byte_index, crate::MalformedKind::ArithmeticOverflow)), s)
    } else {
        let after = Part1MqState { work_units: (s.work_units + 1) as u64, ..s };
        if after.work_units > after.maximum_work_units {
            (Err(part1_mq_limit(crate::Resource::WorkUnits, after.maximum_work_units, after.work_units)), after)
        } else { (Ok(()), after) }
    }
}

pub open spec fn part1_mq_set_context(s: Part1MqState, index: usize, state: u8, mps: bool) -> Part1MqEffect<()> {
    if index >= s.contexts.len() || state >= 47 {
        (Err(part1_mq_error(s.byte_index, crate::MalformedKind::InvalidMqState)), s)
    } else {
        (Ok(()), Part1MqState { contexts: s.contexts.update(index as int, (state, mps)), ..s })
    }
}

pub open spec fn part1_mq_transition(s: Part1MqState, index: usize, state: u8, switch: bool) -> Part1MqEffect<()> {
    if index >= s.contexts.len() || state >= 47 {
        (Err(part1_mq_error(s.byte_index, crate::MalformedKind::InvalidMqState)), s)
    } else {
        part1_mq_set_context(s, index, state, s.contexts[index as int].1 != switch)
    }
}

pub open spec fn part1_mq_byte_in(s: Part1MqState) -> Part1MqEffect<()> {
    let current = part1_mq_current(s);
    let next = part1_mq_next(s);
    let advanced = if super::part1_mq_byte_advances(current, next) { part1_mq_advance(s) } else { (Ok(()), s) };
    match advanced.0 {
        Err(error) => (Err(error), advanced.1),
        Ok(()) => {
            let after = advanced.1;
            let addend = super::part1_mq_byte_addend(current, next);
            if after.c as nat + addend as nat > u32::MAX {
                (Err(part1_mq_error(after.byte_index, crate::MalformedKind::ArithmeticOverflow)), after)
            } else {
                let loaded = Part1MqState {
                    c: (after.c + addend) as u32,
                    ct: super::part1_mq_byte_ct(current, next),
                    ..after
                };
                if super::part1_mq_byte_class(current, next) == 0 {
                    if loaded.marker_padding_count == u64::MAX {
                        (Err(part1_mq_error(loaded.byte_index, crate::MalformedKind::ArithmeticOverflow)), loaded)
                    } else {
                        (Ok(()), Part1MqState { marker_padding_count: (loaded.marker_padding_count + 1) as u64, ..loaded })
                    }
                } else { (Ok(()), loaded) }
            }
        }
    }
}

pub open spec fn part1_mq_renorm_step(s: Part1MqState) -> Part1MqEffect<()> {
    let loaded = if s.ct == 0 { part1_mq_byte_in(s) } else { (Ok(()), s) };
    match loaded.0 {
        Err(error) => (Err(error), loaded.1),
        Ok(()) => {
            let t = loaded.1;
            if t.a > u32::MAX / 2 || t.c > u32::MAX / 2 || t.ct == 0 {
                (Err(part1_mq_error(t.byte_index, crate::MalformedKind::ArithmeticOverflow)), t)
            } else {
                part1_mq_charge(Part1MqState {
                    a: (t.a * 2) as u32, c: (t.c * 2) as u32, ct: (t.ct - 1) as u8, ..t
                })
            }
        }
    }
}

pub open spec fn part1_mq_renormalize(s: Part1MqState, remaining: nat) -> Part1MqEffect<()>
    decreases remaining,
{
    if s.a >= 0x8000 { (Ok(()), s) }
    else if remaining == 0 { (Err(part1_mq_error(s.byte_index, crate::MalformedKind::InvalidMqState)), s) }
    else {
        let next = part1_mq_renorm_step(s);
        match next.0 {
            Err(error) => (Err(error), next.1),
            Ok(()) => part1_mq_renormalize(next.1, (remaining - 1) as nat),
        }
    }
}

pub open spec fn part1_mq_exchange(s: Part1MqState, index: usize, p: Part1MqProbability, lower: bool) -> Part1MqEffect<bool>
    recommends index < s.contexts.len(),
{
    let lps = if lower { s.a >= p.0 as u32 } else { s.a < p.0 as u32 };
    let decision = s.contexts[index as int].1 != lps;
    let interval = if lower { Part1MqState { a: p.0 as u32, ..s } } else { s };
    let transition = part1_mq_transition(interval, index, if lps { p.2 } else { p.1 }, lps && p.3);
    match transition.0 {
        Err(error) => (Err(error), transition.1),
        Ok(()) => {
            let renorm = part1_mq_renormalize(transition.1, 16);
            match renorm.0 {
                Err(error) => (Err(error), renorm.1),
                Ok(()) => (Ok(decision), renorm.1),
            }
        }
    }
}

pub open spec fn part1_mq_decode(s: Part1MqState, index: usize) -> Part1MqEffect<bool> {
    if index >= s.contexts.len() {
        (Err(part1_mq_error(s.byte_index, crate::MalformedKind::InvalidMqState)), s)
    } else {
        let charged = part1_mq_charge(s);
        match charged.0 {
            Err(error) => (Err(error), charged.1),
            Ok(()) => {
                let t = charged.1;
                let context = t.contexts[index as int];
                if context.0 >= 47 {
                    (Err(part1_mq_error(t.byte_index, crate::MalformedKind::InvalidMqState)), t)
                } else {
                    let p = part1_mq_probabilities()[context.0 as int];
                    if p.0 as u32 > t.a {
                        (Err(part1_mq_error(t.byte_index, crate::MalformedKind::InvalidMqState)), t)
                    } else {
                        let interval = Part1MqState { a: (t.a - p.0) as u32, ..t };
                        if (t.c >> 16) < p.0 as u32 {
                            part1_mq_exchange(interval, index, p, true)
                        } else if p.0 as nat * 65536 > t.c as nat {
                            (Err(part1_mq_error(interval.byte_index, crate::MalformedKind::InvalidMqState)), interval)
                        } else {
                            let code = Part1MqState { c: (t.c - p.0 as int * 65536) as u32, ..interval };
                            if interval.a & 0x8000 == 0 {
                                part1_mq_exchange(code, index, p, false)
                            } else { (Ok(context.1), code) }
                        }
                    }
                }
            }
        }
    }
}

pub open spec fn part1_mq_bytes_consumed(s: Part1MqState) -> usize {
    if s.byte_index >= s.input.len() { s.input.len() as usize } else { (s.byte_index + 1) as usize }
}

pub open spec fn part1_mq_termination(s: Part1MqState) -> Result<(), crate::DecodeError> {
    if s.input.len() - part1_mq_bytes_consumed(s) > 2 || s.marker_padding_count > 2 {
        Err(part1_mq_error(s.byte_index, crate::MalformedKind::InvalidCodeBlock))
    } else { Ok(()) }
}

pub open spec fn part1_mq_initial(input: Seq<u8>, maximum_input_bytes: u64, maximum_work_units: u64)
    -> Result<Part1MqState, crate::DecodeError>
{
    if input.len() > maximum_input_bytes {
        Err(part1_mq_limit(crate::Resource::InputBytes, maximum_input_bytes, input.len() as u64))
    } else {
        let before = Part1MqState {
            input, byte_index: 0, a: 0x8000,
            c: if input.len() == 0 { 0xff0000 } else { (input[0] as u32) << 16 },
            ct: 0, contexts: Seq::new(19, |i: int| (0u8, false)),
            work_units: 0, maximum_work_units, marker_padding_count: 0,
        };
        let loaded = part1_mq_byte_in(before);
        match loaded.0 {
            Err(error) => Err(error),
            Ok(()) => {
                let s = loaded.1;
                if s.c as nat * 128 > u32::MAX {
                    Err(part1_mq_error(0, crate::MalformedKind::ArithmeticOverflow))
                } else if s.ct < 7 {
                    Err(part1_mq_error(0, crate::MalformedKind::InvalidMqState))
                } else {
                    Ok(Part1MqState { c: (s.c * 128) as u32, ct: (s.ct - 7) as u8, ..s })
                }
            }
        }
    }
}

}
