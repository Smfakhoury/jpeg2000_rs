verus! {
    pub closed spec fn prepare_plan_view(plan: &PlannedTileComponent) -> crate::specs::Part1PreparePlan {
        crate::specs::Part1PreparePlan {
            component: plan.component, x: plan.x, y: plan.y, width: plan.width, height: plan.height,
            resolution: plan.output_resolution, subbands: plan.subbands@, placements: plan.placements@,
        }
    }

    pub closed spec fn prepare_style_view(style: CodingStyle) -> crate::specs::Part1PrepareStyle {
        let selector = crate::codestream::decomposition_selector_view(style);
        (style.transform, selector.0, selector.1)
    }

    pub open spec fn prepare_precincts_view(precincts: Seq<PacketPrecinctDecoder>) -> Seq<Seq<crate::specs::Tier1RawPlane>> {
        Seq::new(precincts.len(), |i: int| {
            let decoders = crate::packet::packet_precinct_decoders(&precincts[i]);
            Seq::new(decoders.len(), |j: int| crate::tier1::tier1_decoder_plane_view(&decoders[j]))
        })
    }

    pub proof fn planning_precinct_planes_known_fields(values: Seq<PacketPrecinctDecoder>)
        ensures prepare_precincts_view(values) == crate::specs::part2_precinct_decoder_planes(planning_precincts_view(values)),
    {
        assert forall |i: int| 0 <= i < values.len() implies
            prepare_precincts_view(values)[i]
                == crate::specs::part2_precinct_decoder_planes(planning_precincts_view(values))[i] by {
            crate::packet::packet_precinct_decoder_states(&values[i]);
            let decoders = crate::packet::packet_precinct_decoders(&values[i]);
            assert forall |j: int| 0 <= j < decoders.len() implies
                crate::tier1::tier1_decoder_plane_view(&decoders[j])
                    == crate::tier1::tier1_decoder_state(&decoders[j]).plane by {
                crate::tier1::tier1_decoder_plane_matches_state(&decoders[j]);
            }
            assert(prepare_precincts_view(values)[i]
                =~= crate::specs::part2_precinct_decoder_planes(planning_precincts_view(values))[i]);
        }
        assert(prepare_precincts_view(values) =~= crate::specs::part2_precinct_decoder_planes(planning_precincts_view(values)));
    }

    pub closed spec fn decoded_tile_components_view(value: &DecodedTileComponents) -> crate::specs::Part2DecodedTileComponents {
        crate::specs::Part2DecodedTileComponents { prepared: prepared_components_view(value.prepared@), mct_work: value.mct_work }
    }

    pub open spec fn decoded_tile_components_outcome(result: Result<DecodedTileComponents, DecodeError>)
        -> Result<crate::specs::Part2DecodedTileComponents, DecodeError>
    {
        match result { Err(error) => Err(error), Ok(value) => Ok(decoded_tile_components_view(&value)) }
    }

    pub closed spec fn prepared_component_view(value: PreparedTileComponent) -> crate::specs::Part1AssemblyTile {
        (value.component, value.x, value.y, value.width, value.height, value.fraction_bits, value.samples@)
    }

    pub open spec fn prepared_components_view(values: Seq<PreparedTileComponent>) -> Seq<crate::specs::Part1AssemblyTile> {
        Seq::new(values.len(), |index: int| prepared_component_view(values[index]))
    }

    pub open spec fn tile_mct_option(value: Option<&codestream::MultipleComponentTransform>) -> Option<Seq<crate::specs::Part2MctStage>> {
        match value { None => None, Some(transform) => Some(codestream::multiple_component_transform_view(transform)) }
    }

    pub open spec fn tile_part_mct_view(first: &TilePartPayload) -> Option<Seq<crate::specs::Part2MctStage>> {
        match first.multiple_component_transform {
            None => None,
            Some(transform) => Some(codestream::multiple_component_transform_view(&transform)),
        }
    }

    pub closed spec fn planned_components_view(plan: &ProgressionTilePlan) -> Seq<crate::specs::Part1PreparePlan> {
        Seq::new(plan.components@.len(), |index: int| prepare_plan_view(&plan.components@[index]))
    }

    pub closed spec fn progression_reconstruction_view(plan: &ProgressionTilePlan)
        -> (Seq<crate::specs::Part1PreparePlan>, Seq<Seq<crate::specs::Tier1RawPlane>>, ComponentCodingStyles, CodingStyle) {
        (planned_components_view(plan), prepare_precincts_view(plan.precincts@), plan.component_coding_styles, plan.coding_style)
    }

    spec fn prepared_tiles_outcome(result: Result<Vec<TileComponent<'_>>, DecodeError>)
        -> Result<Seq<crate::specs::Part1AssemblyTile>, DecodeError> {
        match result { Ok(tiles) => Ok(crate::image::assembly_tiles_view(tiles@)), Err(error) => Err(error) }
    }

    pub open spec fn prepared_component_outcome(result: Result<PreparedTileComponent, DecodeError>)
        -> Result<crate::specs::Part1AssemblyTile, DecodeError>
    {
        match result { Ok(value) => Ok(prepared_component_view(value)), Err(error) => Err(error) }
    }

    closed spec fn working_subband_view(value: WorkingSubband) -> crate::specs::Part1WorkingSubbandModel {
        (value.width, value.height, value.fraction_bits, value.samples@, value.visited@)
    }

    impl View for WorkingSubband {
        type V = crate::specs::Part1WorkingSubbandModel;

        closed spec fn view(&self) -> Self::V {
            working_subband_view(*self)
        }
    }

        closed spec fn working_bands_view(values: Seq<WorkingSubband>) -> Seq<crate::specs::Part1WorkingSubbandModel> {
            Seq::new(values.len(), |i: int| working_subband_view(values[i]))
        }

        impl View for PrepareWorking {
            type V = crate::specs::Part1PrepareWorking;
            closed spec fn view(&self) -> Self::V { (working_bands_view(self.bands@), self.bytes) }
        }

        closed spec fn prepare_working_outcome(result: Result<PrepareWorking, DecodeError>)
            -> Result<crate::specs::Part1PrepareWorking, DecodeError>
        {
            match result { Ok(value) => Ok(value@), Err(error) => Err(error) }
        }

        impl View for PrepareFold {
            type V = crate::specs::Part1PrepareFold;
            closed spec fn view(&self) -> Self::V { (self.width, self.height, self.samples@, self.first_detail) }
        }

        closed spec fn prepare_fold_outcome(result: Result<PrepareFold, DecodeError>)
            -> Result<crate::specs::Part1PrepareFold, DecodeError>
        {
            match result { Ok(value) => Ok(value@), Err(error) => Err(error) }
        }

        closed spec fn prepare_cell_outcome(result: Result<SignedMagnitude, DecodeError>)
            -> Result<crate::specs::Part1QuantizedCoefficient, DecodeError>
        {
            match result { Ok(value) => Ok((crate::magnitude::magnitude_limbs(value.magnitude), value.negative)), Err(error) => Err(error) }
        }
}
