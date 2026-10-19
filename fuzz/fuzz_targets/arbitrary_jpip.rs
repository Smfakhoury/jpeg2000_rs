#![no_main]

use jpeg2000_rs::{
    DatabinCache, DatabinId, JpipDatabinRange, JpipFragment, JpipHeaderContext,
    ingest_jpip_response, parse_jpip_message,
};
use jpeg2000_rs_fuzz::resource_limits;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let limits = resource_limits();
    let _ = parse_jpip_message(data, &mut JpipHeaderContext::new(), &limits);

    let mut parsed_cache = DatabinCache::new(limits);
    let _ = ingest_jpip_response(data, &mut parsed_cache);
    let _ = parsed_cache.derive_precinct_map(0);
    let _ = parsed_cache.tile_part_transitions(0, 0);
    let _ = parsed_cache.assemble_semantic_container(0);

    let id = DatabinId::new(8, 0, 0).unwrap();
    let split = data.len() / 2;
    let first = &data[..split];
    let second = &data[split..];
    let mut cache = DatabinCache::new(limits);
    if let Ok(later) = JpipFragment::new(id, split as u64, second.to_vec(), true, None, &limits) {
        let _ = cache.insert(later);
    }
    if !first.is_empty()
        && let Ok(earlier) = JpipFragment::new(id, 0, first.to_vec(), false, None, &limits)
    {
        let _ = cache.insert(earlier);
    }
    if !data.is_empty() {
        assert_eq!(cache.databin_view(id).unwrap().as_bytes(), data);
    }

    if data.len() > 2 {
        let overlap = &data[split.saturating_sub(1)..];
        if let Ok(duplicate) = JpipFragment::new(
            id,
            split.saturating_sub(1) as u64,
            overlap.to_vec(),
            true,
            None,
            &limits,
        ) {
            let _ = cache.insert(duplicate);
        }
    }

    if !data.is_empty() {
        let requested = JpipDatabinRange::new(id, 0, data.len() as u64).unwrap();
        let report = cache.availability(&[requested]).unwrap();
        assert!(report.available_bytes() <= report.required_bytes());
        for missing in report.missing_ranges() {
            assert_eq!(missing.databin(), id);
            assert!(missing.length().is_some());
        }
    }
});
