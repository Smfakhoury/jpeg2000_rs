use super::{Part1PacketReaderState, part1_packet_reader_error, part1_packet_reader_read};
use vstd::prelude::*;

verus! {

pub struct Part1TagNode {
    pub low: u32,
    pub value: u32,
}

pub struct Part1TagLevel {
    pub offset: usize,
    pub width: u32,
    pub height: u32,
}

pub struct Part1TagTree {
    pub levels: Seq<Part1TagLevel>,
    pub nodes: Seq<Part1TagNode>,
}

pub struct Part1TagPath {
    pub nodes: Seq<usize>,
    pub length: usize,
}

pub open spec fn part1_tag_leaf_count(tree: Part1TagTree) -> Result<u64, crate::DecodeError> {
    if tree.levels.len() == 0 { Err(part1_packet_reader_error(0, crate::MalformedKind::InvalidTagTree)) }
    else {
        let area = tree.levels[0].width as nat * tree.levels[0].height as nat;
        if area > u64::MAX { Err(part1_packet_reader_error(0, crate::MalformedKind::ArithmeticOverflow)) }
        else { Ok(area as u64) }
    }
}

pub open spec fn part1_tag_path_walk(
    tree: Part1TagTree, path: Part1TagPath, x: u32, y: u32, index: nat, error_offset: usize,
) -> Result<Part1TagPath, crate::DecodeError>
    decreases tree.levels.len() - index,
{
    if index >= tree.levels.len() { Ok(path) }
    else if path.length >= 64 { Err(part1_packet_reader_error(error_offset, crate::MalformedKind::InvalidTagTree)) }
    else {
        let level = tree.levels[index as int];
        if x >= level.width || y >= level.height {
            Err(part1_packet_reader_error(error_offset, crate::MalformedKind::InvalidTagTree))
        } else {
            let row = y as nat * level.width as nat;
            let relative = row + x as nat;
            let node = level.offset as nat + relative;
            if y as nat > usize::MAX || level.width as nat > usize::MAX || row > usize::MAX
                || x as nat > usize::MAX || relative > usize::MAX || node > usize::MAX {
                Err(part1_packet_reader_error(0, crate::MalformedKind::ArithmeticOverflow))
            } else if node >= tree.nodes.len() {
                Err(part1_packet_reader_error(error_offset, crate::MalformedKind::InvalidTagTree))
            } else {
                part1_tag_path_walk(tree,
                    Part1TagPath { nodes: path.nodes.update(path.length as int, node as usize), length: (path.length + 1) as usize },
                    x / 2, y / 2, index + 1, error_offset)
            }
        }
    }
}

#[verifier::opaque]
pub open spec fn part1_tag_path(tree: Part1TagTree, x: u32, y: u32, error_offset: usize)
    -> Result<Part1TagPath, crate::DecodeError>
{
    if tree.levels.len() == 0 { Err(part1_packet_reader_error(0, crate::MalformedKind::InvalidTagTree)) }
    else if x >= tree.levels[0].width || y >= tree.levels[0].height {
        Err(part1_packet_reader_error(error_offset, crate::MalformedKind::InvalidTagTree))
    } else {
        part1_tag_path_walk(tree, Part1TagPath { nodes: Seq::new(64, |i: int| 0usize), length: 0 }, x, y, 0, error_offset)
    }
}

pub open spec fn part1_tag_node_loop(node: Part1TagNode, threshold: u32, reader: Part1PacketReaderState)
    -> (Result<Option<u32>, crate::DecodeError>, Part1TagNode, Part1PacketReaderState)
    decreases threshold as int - node.low as int, node.value as int - node.low as int,
{
    if node.low >= threshold || node.low >= node.value {
        (Ok(if node.value >= threshold { None } else { Some(node.value) }), node, reader)
    } else {
        let bit = part1_packet_reader_read(reader);
        match bit.0 {
            Err(error) => (Err(error), node, bit.1),
            Ok(value) => part1_tag_node_loop(Part1TagNode {
                low: if value { node.low } else { (node.low + 1) as u32 },
                value: if value { node.low } else { node.value },
            }, threshold, bit.1),
        }
    }
}

pub open spec fn part1_tag_node_decode(node: Part1TagNode, inherited: u32, threshold: u32, reader: Part1PacketReaderState)
    -> (Result<Option<u32>, crate::DecodeError>, Part1TagNode, Part1PacketReaderState)
{
    part1_tag_node_loop(Part1TagNode { low: if node.low < inherited { inherited } else { node.low }, ..node }, threshold, reader)
}

pub open spec fn part1_tag_finish(tree: Part1TagTree, path: Part1TagPath, reader: Part1PacketReaderState, threshold: u32)
    -> (Result<crate::packet::TagTreeValue, crate::DecodeError>, Part1TagTree, Part1PacketReaderState)
{
    let leaf = path.nodes[0];
    if leaf >= tree.nodes.len() { (Err(part1_packet_reader_error(reader.byte_index, crate::MalformedKind::InvalidTagTree)), tree, reader) }
    else {
        let value = tree.nodes[leaf as int].value;
        (Ok(crate::packet::TagTreeValue { value: if value == u32::MAX { None } else { Some(value) }, is_below_threshold: value < threshold }), tree, reader)
    }
}

pub open spec fn part1_tag_reverse(
    tree: Part1TagTree, path: Part1TagPath, index: nat, inherited: u32, threshold: u32, reader: Part1PacketReaderState,
) -> (Result<crate::packet::TagTreeValue, crate::DecodeError>, Part1TagTree, Part1PacketReaderState)
    decreases index,
{
    if index == 0 { part1_tag_finish(tree, path, reader, threshold) }
    else if index - 1 >= 64 {
        (Err(part1_packet_reader_error(reader.byte_index, crate::MalformedKind::InvalidTagTree)), tree, reader)
    } else {
        let node_index = path.nodes[index as int - 1];
        if node_index >= tree.nodes.len() {
            (Err(part1_packet_reader_error(reader.byte_index, crate::MalformedKind::InvalidTagTree)), tree, reader)
        } else {
            let decoded = part1_tag_node_decode(tree.nodes[node_index as int], inherited, threshold, reader);
            let tree = Part1TagTree { nodes: tree.nodes.update(node_index as int, decoded.1), ..tree };
            match decoded.0 {
                Err(error) => (Err(error), tree, decoded.2),
                Ok(None) => (Ok(crate::packet::TagTreeValue { value: None, is_below_threshold: false }), tree, decoded.2),
                Ok(Some(value)) => part1_tag_reverse(tree, path, (index - 1) as nat, value, threshold, decoded.2),
            }
        }
    }
}

#[verifier::opaque]
pub open spec fn part1_tag_tree_decode(tree: Part1TagTree, x: u32, y: u32, threshold: u32, reader: Part1PacketReaderState)
    -> (Result<crate::packet::TagTreeValue, crate::DecodeError>, Part1TagTree, Part1PacketReaderState)
{
    match part1_tag_path(tree, x, y, reader.byte_index) {
        Err(error) => (Err(error), tree, reader),
        Ok(path) => part1_tag_reverse(tree, path, path.length as nat, 0, threshold, reader),
    }
}

pub struct Part1TagBuild {
    pub levels: Seq<Part1TagLevel>,
    pub total: usize,
}

pub open spec fn part1_tag_build(
    width: u32, height: u32, before: Part1TagBuild, remaining: nat, maximum_nodes: u64,
) -> Result<Part1TagBuild, crate::DecodeError>
    decreases remaining,
{
    if remaining == 0 { Err(part1_packet_reader_error(0, crate::MalformedKind::ArithmeticOverflow)) }
    else {
        let area = width as nat * height as nat;
        let total = before.total as nat + area;
        if area > u64::MAX || total > u64::MAX {
            Err(part1_packet_reader_error(0, crate::MalformedKind::ArithmeticOverflow))
        } else if total > maximum_nodes {
            Err(super::part1_packet_reader_limit(crate::Resource::TagTreeNodes, maximum_nodes, total as u64))
        } else if total > usize::MAX { Err(part1_packet_reader_error(0, crate::MalformedKind::ArithmeticOverflow)) }
        else {
            let after = Part1TagBuild { levels: before.levels.push(Part1TagLevel { offset: before.total, width, height }), total: total as usize };
            if width == 1 && height == 1 { Ok(after) } else {
                part1_tag_build((width / 2 + width % 2) as u32, (height / 2 + height % 2) as u32,
                    after, (remaining - 1) as nat, maximum_nodes)
            }
        }
    }
}

pub open spec fn part1_tag_initial(width: u32, height: u32, maximum_nodes: u64) -> Result<Part1TagTree, crate::DecodeError> {
    if width == 0 || height == 0 { Err(part1_packet_reader_error(0, crate::MalformedKind::InvalidTagTree)) }
    else {
        match part1_tag_build(width, height, Part1TagBuild { levels: Seq::empty(), total: 0 }, 64, maximum_nodes) {
            Err(error) => Err(error),
            Ok(built) => Ok(Part1TagTree { levels: built.levels, nodes: Seq::new(built.total as nat, |i: int| Part1TagNode { low: 0, value: u32::MAX }) }),
        }
    }
}

}
