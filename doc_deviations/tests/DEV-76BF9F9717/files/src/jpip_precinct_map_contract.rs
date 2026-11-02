// Included in jpip::tests to retain and reuse the native regression fixtures.
#[derive(Clone, Debug)]
struct MapContractStyle {
    levels: u8,
    precincts: Option<Vec<u8>>,
}

impl MapContractStyle {
    fn new(levels: u8, explicit: bool, phase: u8) -> Self {
        Self {
            levels,
            precincts: explicit.then(|| {
                (0..=levels)
                    .map(|r| {
                        if r == 0 && phase == 0 {
                            0
                        } else {
                            (2 + (r + phase) % 2) | ((2 + (r + phase + 1) % 2) << 4)
                        }
                    })
                    .collect()
            }),
        }
    }

    fn sp(&self) -> Vec<u8> {
        assert!(self.levels <= 3);
        let mut bytes = vec![self.levels, 0, 0, 0, 1];
        if let Some(precincts) = &self.precincts {
            assert_eq!(precincts.len(), usize::from(self.levels) + 1);
            for (r, &pp) in precincts.iter().enumerate() {
                assert!(r == 0 || (pp & 15 != 0 && pp >> 4 != 0));
            }
            bytes.extend_from_slice(precincts);
        }
        bytes
    }

    fn cod(&self, progression: u8) -> Vec<u8> {
        assert!(progression <= 4);
        let mut bytes = vec![u8::from(self.precincts.is_some()), progression, 0, 1, 0];
        bytes.extend(self.sp());
        segment(0xff52, &bytes)
    }

    fn coc(&self, component: u8) -> Vec<u8> {
        let mut bytes = vec![component, u8::from(self.precincts.is_some())];
        bytes.extend(self.sp());
        segment(0xff53, &bytes)
    }

    fn quantization(&self, component: Option<u8>) -> Vec<u8> {
        let mut bytes = component.into_iter().collect::<Vec<_>>();
        bytes.push(0x40); // Reversible, two guard bits, one exponent per sub-band.
        bytes.push(0x40);
        for _ in 0..self.levels {
            bytes.extend_from_slice(&[0x48, 0x48, 0x50]);
        }
        segment(if component.is_some() { 0xff5d } else { 0xff5c }, &bytes)
    }
}

#[derive(Clone, Debug)]
struct MapContractGeometry {
    size: [u32; 2],
    origin: [u32; 2],
    tile: [u32; 2],
    tile_origin: [u32; 2],
    sampling: Vec<[u8; 2]>,
    style: MapContractStyle,
    overrides: bool,
}

impl MapContractGeometry {
    fn tile_counts(&self) -> [u32; 2] {
        std::array::from_fn(|axis| {
            assert!(self.size[axis] > 0 && self.tile[axis] > 0);
            assert!(self.tile_origin[axis] <= self.origin[axis]);
            assert!(self.origin[axis] < self.tile_origin[axis] + self.tile[axis]);
            (self.origin[axis] + self.size[axis] - self.tile_origin[axis]).div_ceil(self.tile[axis])
        })
    }

    fn fixture(
        &self,
        stream: u64,
        progression: u8,
        limits: ResourceLimits,
    ) -> (DatabinCache, Vec<(DatabinId, Vec<u8>)>) {
        // Part 1 A.5.1/A.6.1/A.6.2: unrestricted Rsiz=0, one SIZ
        // immediately after SOC, no CAP/Part 2/JP3D, no MCT or packed headers.
        assert!(!self.sampling.is_empty() && self.sampling.len() < 257);
        let mut siz = 0_u16.to_be_bytes().to_vec();
        for pair in [
            std::array::from_fn(|a| self.origin[a] + self.size[a]),
            self.origin,
            self.tile,
            self.tile_origin,
        ] {
            for value in pair {
                siz.extend_from_slice(&value.to_be_bytes());
            }
        }
        siz.extend_from_slice(&u16::try_from(self.sampling.len()).unwrap().to_be_bytes());
        for &[x, y] in &self.sampling {
            assert!(x > 0 && y > 0);
            siz.extend_from_slice(&[7, x, y]);
        }
        let mut main = SOC.to_vec();
        main.extend(segment(0xff51, &siz));
        assert_eq!(
            &main[2..6],
            &[
                0xff,
                0x51,
                0,
                u8::try_from(38 + 3 * self.sampling.len()).unwrap()
            ]
        );
        main.extend(self.style.cod(progression));
        main.extend(self.style.quantization(None));
        if self.overrides {
            assert!(self.sampling.len() >= 2);
            let style = MapContractStyle::new(1, true, 1);
            main.extend(style.coc(1));
            main.extend(style.quantization(Some(1)));
        }
        let counts = self.tile_counts();
        let tile_count = counts[0] * counts[1];
        assert!(tile_count > 0 && tile_count < 32);
        let mut bins = vec![(DatabinId::new(6, stream, 0).unwrap(), main)];
        for tile in 0..tile_count {
            let mut body = Vec::new();
            if self.overrides && tile % 2 == 0 {
                let style = MapContractStyle::new(2, true, 2);
                body.extend(style.cod(progression));
                body.extend(style.quantization(None));
            }
            if self.overrides && tile % 3 == 0 {
                let style = MapContractStyle::new(0, true, 0);
                body.extend(style.coc(1));
                body.extend(style.quantization(Some(1)));
            }
            let mut sot = u16::try_from(tile).unwrap().to_be_bytes().to_vec();
            // Psot includes one byte per empty packet in the source codestream,
            // even though JPP transmits those packets in separate precinct bins.
            let packet_bytes = self
                .expected()
                .iter()
                .filter(|m| u32::from(m.0) == tile)
                .count();
            let tile_part_length = u32::try_from(14 + body.len() + packet_bytes).unwrap();
            sot.extend_from_slice(&tile_part_length.to_be_bytes());
            sot.extend_from_slice(&[0, 1]); // Only the first tile-part carries overrides.
            let mut header = segment(0xff90, &sot);
            header.extend(body);
            header.extend_from_slice(&[0xff, 0x93]);
            assert_eq!(header.len() + packet_bytes, tile_part_length as usize);
            bins.push((DatabinId::new(2, stream, u64::from(tile)).unwrap(), header));
        }
        let mut cache = DatabinCache::new(limits);
        for (id, bytes) in &bins {
            cache.insert(fragment(*id, 0, bytes, true)).unwrap();
            assert!(cache.is_complete(*id));
        }
        (cache, bins)
    }

    fn expected(&self) -> Vec<(u16, u16, u8, u32, u64)> {
        let counts = self.tile_counts();
        let tiles = counts[0] * counts[1];
        let mut expected = Vec::new();
        for tile in 0..tiles {
            for (component, sampling) in self.sampling.iter().enumerate() {
                // T.800 A.6 precedence, determined solely from the input markers.
                let mut style = self.style.clone();
                if self.overrides && component == 1 {
                    style = MapContractStyle::new(1, true, 1);
                }
                if self.overrides && tile % 2 == 0 {
                    style = MapContractStyle::new(2, true, 2);
                }
                if self.overrides && tile % 3 == 0 && component == 1 {
                    style = MapContractStyle::new(0, true, 0);
                }
                let mut sequence = 0_u64;
                for resolution in 0..=style.levels {
                    let pp = style
                        .precincts
                        .as_ref()
                        .map_or(0xff, |p| p[usize::from(resolution)]);
                    let scale = 1 << (style.levels - resolution);
                    let mut spans = [0; 2];
                    for axis in 0..2 {
                        let tile_axis = if axis == 0 {
                            tile % counts[0]
                        } else {
                            tile / counts[0]
                        };
                        let start = self.tile_origin[axis] + tile_axis * self.tile[axis];
                        let end =
                            (start + self.tile[axis]).min(self.origin[axis] + self.size[axis]);
                        let start = start.max(self.origin[axis]);
                        let lo = start.div_ceil(u32::from(sampling[axis])).div_ceil(scale);
                        let hi = end.div_ceil(u32::from(sampling[axis])).div_ceil(scale);
                        let width = 1 << if axis == 0 { pp & 15 } else { pp >> 4 };
                        // T.800 B-16 explicitly gives zero for empty resolution bounds.
                        spans[axis] = if hi == lo {
                            0
                        } else {
                            hi.div_ceil(width) - lo / width
                        };
                    }
                    for precinct in 0..spans[0] * spans[1] {
                        // T.808 A-1, in u128 so implementation overflow cannot define the oracle.
                        let identifier = u128::from(tile)
                            + (component as u128
                                + u128::from(sequence) * self.sampling.len() as u128)
                                * u128::from(tiles);
                        expected.push((
                            u16::try_from(tile).unwrap(),
                            u16::try_from(component).unwrap(),
                            resolution,
                            precinct,
                            u64::try_from(identifier).unwrap(),
                        ));
                        sequence += 1;
                    }
                }
            }
        }
        expected.sort_unstable();
        expected
    }
}

fn map_contract_observe(
    cache: &DatabinCache,
    stream: u64,
) -> Result<Vec<(u16, u16, u8, u32, u64)>, DecodeError> {
    let mapping = cache.derive_precinct_map(stream)?;
    assert_eq!(mapping.codestream(), stream);
    let mut observed = Vec::new();
    let mut ids = std::collections::BTreeSet::new();
    for entry in mapping.mappings() {
        let id = entry.databin();
        assert_eq!(id.class(), DatabinClass::Precinct);
        assert_eq!(id.codestream(), stream);
        assert!(ids.insert(id.in_class()), "duplicate in-class identifier");
        let c = entry.coordinate();
        observed.push((c.tile, c.component, c.resolution, c.precinct, id.in_class()));
    }
    observed.sort_unstable();
    Ok(observed)
}

#[test]
fn derive_precinct_map_matches_standard_geometry_contract() {
    automatic_mapping_decodes_incomplete_precinct_quality_prefixes();
    automatic_mapping_interleaves_multiple_tiles_and_collapses_tile_parts();
    map_contract_packet_limits();

    let mut fixtures = Vec::new();
    for &side in &[1, 3, 4, 5, 7, 8, 9, 15, 16, 17, 31, 32, 33] {
        fixtures.push(MapContractGeometry {
            size: [side, side + 1],
            origin: [0, 0],
            tile: [side, side + 1],
            tile_origin: [0, 0],
            sampling: vec![[1, 1]],
            style: MapContractStyle::new(0, true, 0),
            overrides: false,
        });
    }
    let mut seed = 0x4a50_4950_u32;
    for index in 0..32 {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let width = 25 + seed % 42;
        let height = 25 + (seed >> 8) % 42;
        let origin = [seed % 4, (seed >> 4) % 4];
        fixtures.push(MapContractGeometry {
            size: [width, height],
            origin,
            tile: if index % 2 == 0 {
                [24, 24]
            } else {
                [width + 4, height + 4]
            },
            tile_origin: if index % 3 == 0 { origin } else { [0, 0] },
            sampling: vec![[1, 1], [2, 1], [1, 3]],
            style: MapContractStyle::new(
                if index % 2 == 0 { 2 } else { (index % 4) as u8 },
                index % 3 != 0,
                1,
            ),
            overrides: index % 2 == 0,
        });
    }
    fixtures.push(MapContractGeometry {
        size: [1, 1],
        origin: [1, 1],
        tile: [2, 2],
        tile_origin: [0, 0],
        sampling: vec![[1, 1]],
        style: MapContractStyle::new(3, true, 1),
        overrides: false,
    });
    fixtures.push(MapContractGeometry {
        size: [513, 257],
        origin: [0, 0],
        tile: [513, 257],
        tile_origin: [0, 0],
        sampling: vec![[1, 1], [255, 255]],
        style: MapContractStyle::new(0, false, 1),
        overrides: false,
    });
    let mut failures = Vec::new();
    for (index, geometry) in fixtures.iter().enumerate() {
        let expected = geometry.expected();
        assert!(!expected.is_empty());
        eprintln!(
            "route=DatabinCache::insert -> derive_precinct_map fixture={index} geometry={geometry:?}"
        );
        for progression in 0..=4 {
            let stream = if progression == 4 {
                u64::MAX
            } else {
                17 + index as u64
            };
            let (cache, bins) = geometry.fixture(stream, progression, ResourceLimits::default());
            let observed = match map_contract_observe(&cache, stream) {
                Ok(observed) => observed,
                Err(error) => {
                    let failure = format!(
                        "fixture={index} progression={progression}: expected {} mappings, got {error:?}",
                        expected.len()
                    );
                    eprintln!("{failure}");
                    failures.push(failure);
                    continue;
                }
            };
            if observed != expected {
                let first = observed.iter().zip(&expected).find(|(a, b)| a != b);
                let failure = format!(
                    "fixture={index} progression={progression} stream={stream}: A-1/B-16 mismatch; actual={} expected={} first differing (actual,expected)={first:?}",
                    observed.len(),
                    expected.len()
                );
                eprintln!("{failure}");
                failures.push(failure);
                continue;
            }
            for (id, bytes) in &bins {
                assert_eq!(cache.databin_view(*id).unwrap().as_bytes(), bytes);
            }
            // Header fragmentation and insertion order cannot change geometry.
            // Only iteration order may vary; all named coordinates/IDs must agree.
            let mut chunked = DatabinCache::new(ResourceLimits::default());
            for (id, bytes) in bins.iter().rev() {
                let split = bytes.len() / 2;
                chunked
                    .insert(fragment(*id, split as u64, &bytes[split..], true))
                    .unwrap();
                chunked
                    .insert(fragment(*id, 0, &bytes[..split], false))
                    .unwrap();
                assert!(chunked.is_complete(*id));
            }
            assert_eq!(
                map_contract_observe(&chunked, stream).unwrap(),
                expected,
                "fragmentation invariance"
            );
            assert_eq!(
                map_contract_observe(&cache, stream).unwrap(),
                observed,
                "read-only repeatability"
            );
        }
    }
    assert!(
        failures.is_empty(),
        "{} geometry contract violations; first failures:\n{}",
        failures.len(),
        failures
            .iter()
            .take(8)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}

fn map_contract_packet_limits() {
    // A real planner limit, reached through valid headers, rather than fabricated
    // private state or enormous tile/component counts.
    let geometry = MapContractGeometry {
        size: [9, 7],
        origin: [0, 0],
        tile: [9, 7],
        tile_origin: [0, 0],
        sampling: vec![[1, 1]],
        style: MapContractStyle::new(0, true, 0),
        overrides: false,
    };
    let packets = geometry.expected().len() as u64;
    assert_eq!(packets, 63);
    for limit in [packets - 1, packets] {
        let limits = ResourceLimits::default()
            .with_maximum_packets(limit)
            .unwrap();
        let (cache, bins) = geometry.fixture(7, 0, limits);
        eprintln!("route=derive_precinct_map packet-limit={limit} required={packets}");
        if limit == packets {
            assert_eq!(
                map_contract_observe(&cache, 7).unwrap(),
                geometry.expected()
            );
        } else {
            assert!(
                matches!(
                    cache.derive_precinct_map(7),
                    Err(DecodeError::LimitExceeded {
                        resource: Resource::Packets,
                        limit: actual_limit,
                        actual,
                    }) if actual_limit == limit && actual == packets
                ),
                "must reject exactly one packet beyond the configured bound"
            );
        }
        for (id, bytes) in bins {
            assert!(cache.is_complete(id));
            assert_eq!(cache.databin_view(id).unwrap().as_bytes(), bytes);
        }
    }
}
