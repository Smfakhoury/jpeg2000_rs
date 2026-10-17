use vstd::prelude::*;

verus! {

pub struct CursorView {
    pub position: int,
    pub length: int,
}

impl CursorView {
    pub open spec fn invariant(self) -> bool {
        0 <= self.position && self.position <= self.length
    }

    pub open spec fn remaining(self) -> int
        recommends self.invariant()
    {
        self.length - self.position
    }
}

pub struct DatabinView {
    pub contiguous_prefix: int,
    pub final_length: int,
}

impl DatabinView {
    pub open spec fn invariant(self) -> bool {
        0 <= self.contiguous_prefix && self.contiguous_prefix <= self.final_length
    }

    pub open spec fn complete(self) -> bool
        recommends self.invariant()
    {
        self.contiguous_prefix == self.final_length
    }
}

pub struct PacketBitReaderView {
    pub byte_index: nat,
    pub bits_remaining: nat,
    pub previous_byte_was_ff: bool,
    pub bits_read: nat,
}

impl PacketBitReaderView {
    pub open spec fn invariant(self) -> bool {
        self.bits_remaining <= 8
    }
}

pub struct TagTreeNodeView {
    pub low: nat,
    pub value: nat,
}

impl TagTreeNodeView {
    pub open spec fn invariant(self) -> bool {
        self.low <= self.value
    }

    pub open spec fn monotonic_from(self, old: TagTreeNodeView) -> bool {
        old.low <= self.low && self.value <= old.value
    }
}

pub struct MqContextView {
    pub state: nat,
    pub mps: bool,
}

impl MqContextView {
    pub open spec fn invariant(self) -> bool {
        self.state < 47
    }
}

pub struct Tier1CoefficientView {
    pub magnitude: nat,
    pub significant: bool,
    pub refined: bool,
}

impl Tier1CoefficientView {
    pub open spec fn refinement_from(self, old: Tier1CoefficientView, delta: nat) -> bool {
        old.significant
            && self.significant
            && self.refined
            && (self.magnitude == old.magnitude || self.magnitude == old.magnitude + delta)
    }
}

pub struct SampleView {
    pub value: int,
    pub minimum: int,
    pub maximum: int,
}

impl SampleView {
    pub open spec fn invariant(self) -> bool {
        self.minimum <= self.maximum
            && self.minimum <= self.value
            && self.value <= self.maximum
    }
}

pub struct MarkerParserView {
    pub position: nat,
    pub input_length: nat,
    pub in_tile_header: bool,
}

impl MarkerParserView {
    pub open spec fn invariant(self) -> bool {
        self.position <= self.input_length
    }

    pub open spec fn advances_from(self, old: MarkerParserView) -> bool {
        old.position < self.position
            && self.position <= self.input_length
            && old.input_length == self.input_length
    }
}

pub struct PacketContributionView {
    pub included: bool,
    pub lblock: nat,
    pub total_passes: nat,
    pub bytes: nat,
}

impl PacketContributionView {
    pub open spec fn invariant(self) -> bool {
        3 <= self.lblock && self.lblock <= 31
    }

    pub open spec fn extends(self, old: PacketContributionView) -> bool {
        (!old.included || self.included)
            && old.lblock <= self.lblock
            && old.total_passes <= self.total_passes
            && old.bytes <= self.bytes
    }
}

pub struct MqRegisterView {
    pub interval: nat,
    pub code: nat,
    pub ct: nat,
    pub byte_index: nat,
}

impl MqRegisterView {
    pub open spec fn invariant(self) -> bool {
        self.interval < 65536 && self.ct <= 8
    }
}

pub struct Tier1PassView {
    pub bitplane: nat,
    pub significance_complete: bool,
    pub refinement_complete: bool,
    pub cleanup_complete: bool,
}

impl Tier1PassView {
    pub open spec fn ordered(self) -> bool {
        (!self.refinement_complete || self.significance_complete)
            && (!self.cleanup_complete
                || self.significance_complete == self.refinement_complete)
    }
}

pub struct WaveletPlaneView {
    pub width: nat,
    pub height: nat,
    pub samples: nat,
    pub fraction_bits: nat,
}

impl WaveletPlaneView {
    pub open spec fn rectangular(self) -> bool {
        self.width * self.height == self.samples
    }
}

pub struct Part1DecodeView {
    pub input_bytes: nat,
    pub tile_parts: nat,
    pub components: nat,
    pub width: nat,
    pub height: nat,
}

impl Part1DecodeView {
    pub open spec fn denotation_well_formed(self) -> bool {
        self.input_bytes > 0
            && self.tile_parts > 0
            && self.components > 0
            && self.width > 0
            && self.height > 0
    }
}

}
