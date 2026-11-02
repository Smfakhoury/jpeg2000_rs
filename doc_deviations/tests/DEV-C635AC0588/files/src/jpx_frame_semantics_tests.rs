// T.801 (06/2021) M.5.3.2.1, at the normalized instruction boundary.
// No byte-parser conformance is inferred: Ityp=0x000b denotes position, size,
// and animation fields in Tables M.38/M.40; LIFE is already in milliseconds.
fn closing_semantics_instruction(x: u32, life: u32, persistent: bool) -> CompositionInstruction {
    CompositionInstruction {
        target_x: x,
        target_y: 0,
        target_width: 1,
        target_height: 1,
        life,
        persistent,
        next_reuse: 0,
        source_x: 0,
        source_y: 0,
        source_width: 0,
        source_height: 0,
    }
}

fn closing_semantics_composition(instructions: Vec<CompositionInstruction>) -> Composition {
    Composition {
        width: u32::try_from(instructions.len()).unwrap(),
        height: 1,
        loop_count: 1,
        has_unknown_boxes: false,
        instruction_sets: vec![CompositionInstructionSet {
            flags: 0x000b,
            repeat: 0,
            tick: 1,
            instructions,
        }],
    }
}

fn closing_semantics_render(
    frames: &[JpxFrameInfo],
    width: u32,
    requested: usize,
) -> Result<ConvertedImage, DecodeError> {
    let limits = ResourceLimits::default();
    let bank = (0..width)
        .map(|index| {
            (
                usize::try_from(index).unwrap(),
                Some(Ok(OwnedLayer {
                    width: 1,
                    height: 1,
                    color_space: ConvertedColorSpace::Grayscale,
                    colors: vec![vec![u16::try_from((index + 1) * 1000).unwrap()]],
                    alpha: None,
                })),
            )
        })
        .collect();
    render_normalized_frame(
        width,
        1,
        frames,
        1,
        requested,
        bank,
        limits.maximum_decoded_bytes(),
        &limits,
    )
}

fn assert_frame_closing_semantics() {
    eprintln!(
        "frame-closing route: build_timeline -> JpxFrameInfo observers -> \
         render_normalized_frame -> select_frame_instructions; \
         opaque 1x1 grayscale layers at disjoint canvas columns"
    );
    let mut lives = vec![1, 2, 17, 65_535, 0x7fff_fffe];
    let mut seed = 0x8b31_902d_u32;
    for _ in 0..12 {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        lives.push(1 + seed % 1_000_000);
    }
    for prefix in 0_u32..=3 {
        for closing_persistent in [false, true] {
            for &life in &lives {
                let mut instructions: Vec<_> = (0..prefix)
                    .map(|x| closing_semantics_instruction(x, 0, true))
                    .collect();
                instructions.extend([
                    closing_semantics_instruction(prefix, life, closing_persistent),
                    closing_semantics_instruction(prefix + 1, 13, false),
                    closing_semantics_instruction(prefix + 2, 7, true),
                ]);
                let composition = closing_semantics_composition(instructions);
                assert_eq!(composition.instruction_sets[0].flags, 0x000b);
                assert_eq!(composition.instruction_sets[0].tick, 1);
                assert_eq!(composition.instruction_sets[0].repeat, 0);
                assert!(life > 0 && life < 0x7fff_ffff);
                let frames =
                    build_timeline(&composition, usize::try_from(composition.width).unwrap(), 3)
                        .unwrap();
                let before = frames.clone();
                assert_eq!(
                    frames
                        .iter()
                        .map(|f| (
                            f.index(),
                            f.start_milliseconds(),
                            f.duration_milliseconds(),
                            f.persistent(),
                            f.instructions().len(),
                        ))
                        .collect::<Vec<_>>(),
                    [
                        (0, 0, life, closing_persistent, prefix as usize + 1),
                        (1, u64::from(life), 13, false, 1),
                        (2, u64::from(life) + 13, 7, true, 1),
                    ],
                    "closing metadata: prefix={prefix}, life={life}, persist={closing_persistent}"
                );
                for (index, instruction) in frames[0].instructions().iter().enumerate() {
                    assert_eq!(instruction.layer_index(), index);
                    assert_eq!(instruction.target_x(), index as u32);
                    assert_eq!(
                        instruction.persistent(),
                        index < prefix as usize || closing_persistent
                    );
                }
                for requested in [2, 0, 1, 2, 1, 0] {
                    let image =
                        closing_semantics_render(&frames, composition.width, requested).unwrap();
                    let mut colors = vec![0_u16; composition.width as usize];
                    let mut alpha = vec![0_u16; composition.width as usize];
                    // Closing PERSIST applies to the whole off-screen group.
                    if requested == 0 || closing_persistent {
                        for x in 0..=prefix as usize {
                            colors[x] = (x as u16 + 1) * 1000;
                            alpha[x] = u16::MAX;
                        }
                    }
                    if requested != 0 {
                        let x = prefix as usize + requested;
                        colors[x] = (x as u16 + 1) * 1000;
                        alpha[x] = u16::MAX;
                    }
                    assert_eq!(
                        (image.width(), image.height(), image.precision()),
                        (composition.width, 1, 16)
                    );
                    assert_eq!(image.color_space(), ConvertedColorSpace::Grayscale);
                    assert_eq!(image.channels().len(), 2);
                    assert_eq!(
                        image.channel(0).unwrap().samples(),
                        colors,
                        "pixels: prefix={prefix}, life={life}, persist={closing_persistent}, request={requested}"
                    );
                    assert_eq!(image.channel(1).unwrap().samples(), alpha);
                }
                assert_eq!(
                    closing_semantics_render(&frames, composition.width, 3).err(),
                    Some(DecodeError::FrameIndexOutOfRange {
                        index: 3,
                        frames: 3
                    })
                );
                assert_eq!(
                    frames, before,
                    "observers/rendering must not mutate metadata"
                );
            }
        }
    }

    // End-of-animation also closes a zero-duration composition sequence.
    let tail = closing_semantics_composition(vec![
        closing_semantics_instruction(0, 0, true),
        closing_semantics_instruction(1, 0, true),
    ]);
    let frames = build_timeline(&tail, 2, 1).unwrap();
    assert_eq!(frames.len(), 1);
    assert_eq!(frames[0].start_milliseconds(), 0);
    assert_eq!(frames[0].duration_milliseconds(), 0);
    assert!(frames[0].persistent());
    assert_eq!(frames[0].instructions().len(), 2);
    let image = closing_semantics_render(&frames, 2, 0).unwrap();
    assert_eq!(image.channel(0).unwrap().samples(), [1000, 2000]);
    assert_eq!(image.channel(1).unwrap().samples(), [u16::MAX; 2]);

    // M.5.3.2.1 recommends no action for PERSIST=false/LIFE=0.
    // The skipped thumbnail still consumes its layer assignment, but cannot
    // paint its column or join the visible frame composition sequence.
    let skipped = closing_semantics_composition(vec![
        closing_semantics_instruction(0, 0, true),
        closing_semantics_instruction(1, 0, false),
        closing_semantics_instruction(2, 17, false),
        closing_semantics_instruction(3, 5, true),
    ]);
    let frames = build_timeline(&skipped, 4, 2).unwrap();
    let metadata: Vec<_> = frames
        .iter()
        .map(|f| {
            (
                f.start_milliseconds(),
                f.duration_milliseconds(),
                f.persistent(),
            )
        })
        .collect();
    let first = closing_semantics_render(&frames, 4, 0).unwrap();
    let next = closing_semantics_render(&frames, 4, 1).unwrap();
    let observed = (
        metadata,
        first.channel(0).unwrap().samples().to_vec(),
        first.channel(1).unwrap().samples().to_vec(),
        next.channel(0).unwrap().samples().to_vec(),
        next.channel(1).unwrap().samples().to_vec(),
    );
    eprintln!("zero-LIFE nonpersistent update: observed metadata/pixels/alpha={observed:?}");
    assert_eq!(
        observed,
        (
            vec![(0, 17, false), (17, 5, true)],
            vec![1000, 0, 3000, 0],
            vec![u16::MAX, 0, u16::MAX, 0],
            vec![0, 0, 0, 4000],
            vec![0, 0, 0, u16::MAX],
        ),
        "T.801 M.5.3.2.1: PERSIST=false/LIFE=0 should perform no action; \
         column 1 must remain transparent, not display the skipped thumbnail"
    );
}
