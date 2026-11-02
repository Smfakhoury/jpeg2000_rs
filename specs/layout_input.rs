//! Source-derived model of the actual layout parser's SOC/SIZ prologue.

use super::*;

verus! {

#[verifier::opaque]
pub(crate) open spec fn part1_parse_layout_prologue<'a>(state: Part1CodestreamReader<'a>)
    -> (Result<(Part1ParsedImageSize, u16, usize), crate::DecodeError>, Part1CodestreamReader<'a>)
{
    let soc = part1_codestream_expect_standalone(state, 0xff4f);
    match soc.0 {
        Err(error) => (Err(error), soc.1),
        Ok(()) => {
            let siz = part1_codestream_read_segment(soc.1);
            match siz.0 {
                Err(error) => (Err(error), siz.1),
                Ok(segment) => if segment.marker != 0xff51 {
                    (Err(part1_codestream_malformed(segment.offset, crate::MalformedKind::InvalidMarkerOrder)), siz.1)
                } else {
                    match part1_parse_siz(segment, siz.1.limits) {
                        Err(error) => (Err(error), siz.1),
                        Ok((image_size, r_siz)) => (Ok((image_size, r_siz, siz.1.position)), siz.1),
                    }
                },
            }
        },
    }
}

}
