use super::*;
use vstd::prelude::*;

verus! {

pub struct Part1SinglePacketState {
    pub state:Part1CodeBlockPacketState,
    pub inclusion:Part1TagTree,
    pub zero:Part1TagTree,
    pub decoder:Tier1DecoderState,
}

pub type Part1SinglePacketResult=(Result<Option<crate::packet::CodeBlockBodyDecode>,crate::DecodeError>,Part1SinglePacketState);

pub open spec fn part1_single_packet_body(
    s:Part1SinglePacketState,payload:Seq<u8>,reader:Part1PacketReaderState,contribution:Part1CodeBlockContribution,
    pass_limit:u32,byte_limit:u64,input_limit:u64,work_limit:u64,
) ->Part1SinglePacketResult {
    let offset=reader.byte_index;
    if offset>payload.len() {(Err(part1_packet_reader_error(offset,crate::MalformedKind::Truncated)),s)}
    else {
        let body=part1_code_block_body_transaction(s.decoder,contribution,
            payload.subrange(offset as int,payload.len() as int),pass_limit,byte_limit,input_limit,work_limit);
        (match body.0 {Err(error)=>Err(error),Ok(decoded)=>Ok(Some(decoded))},
            Part1SinglePacketState {decoder:body.1,..s})
    }
}

pub open spec fn part1_single_packet_present(
    s:Part1SinglePacketState,payload:Seq<u8>,reader:Part1PacketReaderState,context:crate::packet::CodeBlockPacketContext,
    pass_limit:u32,byte_limit:u64,input_limit:u64,work_limit:u64,
) ->Part1SinglePacketResult {
    let header=part1_packet_header_decode(s.state,s.inclusion,s.zero,context,reader);
    let s=Part1SinglePacketState {state:header.1,inclusion:header.2,zero:header.3,..s};
    match header.0 {
        Err(error)=>(Err(error),s),
        Ok(None)=>(Err(part1_packet_reader_error(header.4.byte_index,crate::MalformedKind::InvalidPacketHeader)),s),
        Ok(Some(contribution))=>part1_single_packet_body(s,payload,
            Part1PacketReaderState {bits_remaining:0,..header.4},contribution,pass_limit,byte_limit,input_limit,work_limit),
    }
}

pub open spec fn part1_single_code_block_packet(
    s:Part1SinglePacketState,payload:Seq<u8>,context:crate::packet::CodeBlockPacketContext,
    header_limit:u64,pass_limit:u32,byte_limit:u64,input_limit:u64,work_limit:u64,
) ->Part1SinglePacketResult {
    match part1_packet_reader_initial(payload,header_limit,work_limit) {
        Err(error)=>(Err(error),s),
        Ok(reader)=>{
            let first=part1_packet_reader_read(reader);
            match first.0 {
                Err(error)=>(Err(error),s),
                Ok(false)=>if first.1.byte_index!=payload.len() {
                    (Err(part1_packet_reader_error(first.1.byte_index,crate::MalformedKind::TrailingData)),s)
                }else{(Ok(None),s)},
                Ok(true)=>part1_single_packet_present(s,payload,first.1,context,pass_limit,byte_limit,input_limit,work_limit),
            }
        },
    }
}

}
