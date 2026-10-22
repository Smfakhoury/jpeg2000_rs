import copy
import json
import os
import re
import shutil
import subprocess
import sys
import unittest
import uuid
from pathlib import Path
from unittest import mock

VALIDATION = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(VALIDATION))

import framework


class WorkingFiles(unittest.TestCase):
    def setUp(self):
        self.work = framework.RESULTS_DIR / "test-work" / uuid.uuid4().hex
        self.work.mkdir(parents=True)
        self.addCleanup(shutil.rmtree, self.work)

    def subject(self):
        path = self.work / "subject"
        accepted = {
            entry["id"] for entry in framework.load_manifest()["entries"]
            if entry["expected"] == "accept"
        }
        path.write_text(
            f"#!{sys.executable}\n"
            "import sys\nfrom pathlib import Path\n"
            f"accepted = {accepted!r}\n"
            "if Path(sys.argv[-1]).stem in accepted:\n"
            "    print('IMAGE\\n1 1 1\\n1\\t1\\t8\\t0\\tunsigned\\t128')\n"
            "else:\n"
            "    print('ERR\\tMalformedInput\\texpected rejection')\n"
            "    raise SystemExit(1)\n"
        )
        path.chmod(0o700)
        return path

    def oracle_config(self):
        config = json.loads((framework.VALIDATION / "oracles.json").read_text())
        config["oracles"] = config["oracles"][:1]
        path = self.work / "oracles.json"
        path.write_text(json.dumps(config))
        return path

    def external_manifest(self):
        archive = self.work / "suite.bin"
        vector = self.work / "vector.j2k"
        archive.write_bytes(b"synthetic archive fixture, not an official asset")
        vector.write_bytes(b"synthetic vector fixture, not an official asset")
        manifest = {
            "schema_version": 1,
            "profile_id": framework.PROFILE_ID,
            "suite": {
                "source_url": "https://example.invalid/synthetic-suite",
                "acquired_at": "2026-10-18T00:00:00Z",
                "access_status": "authorized-internal",
                "license": {
                    "status": "approved-internal-use",
                    "reviewed_at": "2026-10-18T00:00:00Z",
                    "redistribution_permitted": False,
                    "basis": "synthetic test fixture",
                },
                "archive": {
                    "path": archive.name,
                    "bytes": archive.stat().st_size,
                    "sha256": framework.sha256_file(archive),
                },
                "selection": {
                    "complete": True,
                    "applicable_vector_count": 1,
                    "basis": "synthetic test fixture",
                },
            },
            "entries": [{
                "id": "private-fixture-id",
                "identifier_public": False,
                "path": vector.name,
                "sha256": framework.sha256_file(vector),
                "bytes": vector.stat().st_size,
                "format": "raw",
                "operation": "decode",
                "applicable": True,
                "expected": {"accepted": False, "category": "MalformedInput"},
            }],
        }
        path = self.work / "official-vector-manifest.json"
        path.write_text(json.dumps(manifest))
        return path, manifest


class FrameworkTests(WorkingFiles):
    def test_generation_is_deterministic_and_complete(self):
        first = framework.build_manifest(framework.generated_cases())
        self.assertEqual(first, framework.build_manifest(framework.generated_cases()))
        self.assertEqual(len(first["entries"]), 17)
        self.assertEqual({entry["format"] for entry in first["entries"]}, {"raw", "jp2", "jph", "jpx", "jpip"})
        framework.generate_corpus(check=True)
        self.assertEqual(framework.verify_manifest()["status"], "passed")

    def test_manifest_rejects_bad_hash_and_restricted_assets(self):
        for field, value in (("sha256", "0" * 64), ("classification", "restricted")):
            manifest = framework.load_manifest()
            manifest["entries"][0][field] = value
            with self.subTest(field=field), self.assertRaises(ValueError):
                framework.verify_manifest(manifest)

    def test_normalization_is_stable(self):
        output = b"IMAGE\n1 1 1\n1\t1\t8\t0\tunsigned\t128\n"
        parsed = framework.parse_subject_output(output, b"", 0)
        self.assertTrue(parsed["accepted"])
        self.assertEqual(framework.normalized_hash(parsed), framework.normalized_hash(parsed))
        first = framework.parse_subject_output(b"ERR\tMalformedInput\tat byte 1\n", b"", 1)
        second = framework.parse_subject_output(b"ERR\tMalformedInput\tat byte 9\n", b"", 1)
        self.assertEqual(first, second)

    def test_binary_pnm_normalization(self):
        path = self.work / "output.pnm"
        path.write_bytes(b"P6\n1 1\n255\n\x01\x02\x03")
        parsed = framework.parse_pnm(path)
        self.assertEqual([c["samples"] for c in parsed["image"]["components"]], [[1], [2], [3]])

    def test_inventory_checks_present_items_without_exact_progress_counts(self):
        coverage = json.loads((framework.ROOT / "normative-coverage.json").read_text())
        self.assertEqual(framework.validate_normative_coverage(coverage)["status"], "passed")
        coverage["requirements"] = coverage["requirements"][:1]
        self.assertEqual(framework.validate_normative_coverage(coverage)["status"], "passed")

    def test_inventory_rejects_missing_evidence_and_shadow_only_links(self):
        original = json.loads((framework.ROOT / "normative-coverage.json").read_text())
        for field in ("implementation_refs", "spec_refs", "contract_refs", "proof_refs", "test_refs"):
            coverage = copy.deepcopy(original)
            coverage["requirements"][0][field][0]["symbol"] = "unchecked_shadow"
            with self.subTest(field=field), self.assertRaises(ValueError):
                framework.validate_normative_coverage(coverage)
        coverage = copy.deepcopy(original)
        coverage["requirements"][0]["implementation_refs"] = [{"path": "src/error.rs", "symbol": "DecodeError"}]
        with self.assertRaisesRegex(ValueError, "not used by claimed implementation"):
            framework.validate_normative_coverage(coverage)

    def test_inventory_checks_named_result_equalities_not_just_model_mentions(self):
        coverage = json.loads((framework.ROOT / "normative-coverage.json").read_text())
        coverage["requirements"] = [
            item for item in coverage["requirements"]
            if item["id"] == "part2-jpx-composition-rendering"
        ]
        self.assertEqual(framework.validate_normative_coverage(coverage)["status"], "passed")
        read_text = Path.read_text
        for before, after in (
            ("verus_spec(output =>", "verus_spec(unrelated =>"),
            ("ensures output as nat ==", "ensures 0 as nat <="),
            ("ensures output as nat == crate::specs::part2_source_over_alpha_numerator",
             "ensures output as nat == crate::specs::unlisted_model"),
        ):
            def altered(path, *args, **kwargs):
                text = read_text(path, *args, **kwargs)
                return text.replace(before, after) if path == framework.ROOT / "src/refinement.rs" else text
            with self.subTest(after=after), mock.patch.object(Path, "read_text", altered), \
                    self.assertRaisesRegex(ValueError, "lacks executable Verus contract"):
                framework.validate_normative_coverage(coverage)

    def test_inventory_requires_named_result_and_final_state_in_mutation_relation(self):
        coverage = json.loads((framework.ROOT / "normative-coverage.json").read_text())
        coverage["requirements"] = [
            item for item in coverage["requirements"]
            if item["id"] == "part2-jpx-composition-rendering"
        ]
        coverage["requirements"][0]["contract_refs"] = [{
            "path": "src/refinement.rs", "symbol": "try_store_composition_sample",
        }]
        self.assertEqual(framework.validate_normative_coverage(coverage)["status"], "passed")
        read_text = Path.read_text
        for before, after in (
            ("verus_spec(stored =>", "verus_spec(unrelated =>"),
            ("ensures (stored, final(samples)@)", "ensures (true, final(samples)@)"),
            ("ensures (stored, final(samples)@)", "ensures (stored, old(samples)@)"),
            ("ensures (stored, final(samples)@)", "ensures stored"),
            ("== crate::specs::part2_composition_store", "== crate::specs::unlisted_model"),
        ):
            def altered(path, *args, **kwargs):
                text = read_text(path, *args, **kwargs)
                return text.replace(before, after) if path == framework.ROOT / "src/refinement.rs" else text
            with self.subTest(after=after), mock.patch.object(Path, "read_text", altered), \
                    self.assertRaisesRegex(ValueError, "lacks executable Verus contract"):
                framework.validate_normative_coverage(coverage)

    def test_inventory_requires_known_complete_projection_of_the_bound_result(self):
        coverage = json.loads((framework.ROOT / "normative-coverage.json").read_text())
        coverage["requirements"] = [
            item for item in coverage["requirements"]
            if item["id"] == "part2-jpx-layer-input-construction"
        ]
        self.assertEqual(framework.validate_normative_coverage(coverage)["status"], "passed")
        read_text = Path.read_text
        for before, after in (
            ("ensures layer_channels_outcome(result)", "ensures layer_channels_outcome(unrelated)"),
            ("ensures layer_channels_outcome(result)", "ensures unlisted_projection(result)"),
            ("ensures layer_channels_outcome(result)", "ensures owned_layer_outcome(result)"),
            ("ensures owned_layer_outcome(result)", "ensures layer_channels_outcome(result)"),
            ("ensures owned_layer_outcome(result)", "ensures owned_layer_outcome(unrelated)"),
            ("== crate::specs::part2_layer_input(", "== crate::specs::unlisted_model("),
            ("== crate::specs::part2_owned_layer_input(", "== crate::specs::unlisted_model("),
        ):
            def altered(path, *args, **kwargs):
                text = read_text(path, *args, **kwargs)
                return text.replace(before, after) if path == framework.ROOT / "src/jpx.rs" else text
            with self.subTest(after=after), mock.patch.object(Path, "read_text", altered), \
                    self.assertRaisesRegex(ValueError, "lacks executable Verus contract"):
                framework.validate_normative_coverage(coverage)

    def test_inventory_checks_each_complete_frame_stage_result_projection(self):
        coverage = json.loads((framework.ROOT / "normative-coverage.json").read_text())
        coverage["requirements"] = [
            item for item in coverage["requirements"]
            if item["id"] == "part2-normalized-layer-frame-stages"
        ]
        requirement = coverage["requirements"][0]
        requirement["implementation_refs"] = [
            {"path": "src/jpx.rs", "symbol": "inspect_file"},
            {"path": "src/jpx.rs", "symbol": "render_inspected_frame"},
            {"path": "src/jpx.rs", "symbol": "render_normalized_frame"},
            {"path": "src/jpx.rs", "symbol": "render_decoded_layers"},
            {"path": "src/color.rs", "symbol": "from_composited"},
        ]
        read_text = Path.read_text
        for relative, symbol, projection in (
            ("src/jpx.rs", "render_decoded_layers", "crate::color::converted_image_result"),
            ("src/jpx.rs", "select_frame_instructions", "frame_selection_result"),
            ("src/jpx.rs", "build_timeline", "timeline_frames_result"),
            ("src/jpx.rs", "render_normalized_frame", "crate::color::converted_image_result"),
            ("src/color.rs", "from_layer", "converted_image_result"),
            ("src/color.rs", "from_composited", "converted_image_result"),
        ):
            requirement["contract_refs"] = [{"path": relative, "symbol": symbol}]
            self.assertEqual(framework.validate_normative_coverage(coverage)["status"], "passed")
            text = read_text(framework.ROOT / relative)
            signature = re.search(rf"\bfn\s+{re.escape(symbol)}\b", text).start()
            start = text.rfind("#[cfg_attr", 0, signature)
            contract = text[start:signature]
            for before, after in (
                (f"ensures {projection}(result)", f"ensures {projection}(unrelated)"),
                (f"ensures {projection}(result)", "ensures unlisted_projection(result)"),
                (f"ensures {projection}(result)", "ensures owned_layer_outcome(result)"),
                ("== crate::specs::", "<= crate::specs::"),
            ):
                self.assertIn(before, contract)
                replacement = contract.replace(before, after)

                def altered(path, *args, **kwargs):
                    original = read_text(path, *args, **kwargs)
                    if path == framework.ROOT / relative:
                        return original[:start] + replacement + original[signature:]
                    return original

                with self.subTest(symbol=symbol, after=after), \
                        mock.patch.object(Path, "read_text", altered), \
                        self.assertRaisesRegex(ValueError, "lacks executable Verus contract"):
                    framework.validate_normative_coverage(coverage)

    def test_inventory_checks_complete_reconstruction_and_sequence_projections(self):
        coverage = json.loads((framework.ROOT / "normative-coverage.json").read_text())
        names = {"inverse_quantize_values", "inverse_quantize_plane", "inverse_5_3", "interleave"}
        coverage["requirements"] = [
            item for item in coverage["requirements"]
            if any(ref["path"] == "src/transform.rs"
                   and ref["symbol"].rsplit("::", 1)[-1] in names
                   for ref in item["contract_refs"])
        ]
        self.assertTrue(coverage["requirements"])
        self.assertEqual(framework.validate_normative_coverage(coverage)["status"], "passed")
        read_text = Path.read_text
        mutations = []
        for projection in ("reconstruction_outcome", "sequence_outcome"):
            mutations.extend((
                (f"ensures {projection}(result)", f"ensures {projection}(unrelated)"),
                (f"ensures {projection}(result)", "ensures unlisted_projection(result)"),
                (f"ensures {projection}(result)", "ensures owned_layer_outcome(result)"),
            ))
        for model in ("part1_inverse_integer_quantization", "part1_inverse_quantization",
                      "part1_inverse_five_three", "part1_interleaved"):
            mutations.append((f"== crate::specs::{model}(", "== crate::specs::unlisted_model("))
        original = read_text(framework.ROOT / "src/transform.rs")
        for before, after in mutations:
            self.assertIn(before, original)

            def altered(path, *args, **kwargs):
                text = read_text(path, *args, **kwargs)
                return text.replace(before, after) if path == framework.ROOT / "src/transform.rs" else text

            with self.subTest(after=after), mock.patch.object(Path, "read_text", altered), \
                    self.assertRaisesRegex(ValueError, "lacks executable Verus contract"):
                framework.validate_normative_coverage(coverage)

    def test_inventory_checks_standard_profile_bound_return_projections(self):
        original_coverage = json.loads((framework.ROOT / "normative-coverage.json").read_text())
        source = framework.ROOT / "src/transform.rs"
        original = source.read_text()
        read_text = Path.read_text
        for function, projection, model in (
            ("inverse_9_7", "sequence_outcome", "part1_inverse_nine_seven"),
            ("inverse_horizontal_5_3", "reconstruction_outcome", "part1_inverse_horizontal"),
            ("inverse_horizontal_9_7", "reconstruction_outcome", "part1_inverse_horizontal"),
            ("inverse_vertical_5_3", "reconstruction_outcome", "part1_inverse_vertical"),
            ("inverse_vertical_9_7", "reconstruction_outcome", "part1_inverse_vertical"),
            ("inverse_level_5_3", "reconstruction_outcome", "part1_inverse_level"),
            ("inverse_level_9_7", "reconstruction_outcome", "part1_inverse_level"),
            ("convert_samples", "sequence_outcome", "part1_convert_samples"),
        ):
            coverage = copy.deepcopy(original_coverage)
            coverage["requirements"] = [
                item for item in coverage["requirements"]
                if any(ref == {"path": "src/transform.rs", "symbol": function}
                       for ref in item["contract_refs"])
            ]
            self.assertEqual(len(coverage["requirements"]), 1)
            coverage["requirements"][0]["contract_refs"] = [
                {"path": "src/transform.rs", "symbol": function}
            ]
            with self.subTest(function=function, outcome="complete"):
                self.assertEqual(framework.validate_normative_coverage(coverage)["status"], "passed")
            end = original.index(f"pub fn {function}(")
            start = original.rfind("#[cfg_attr", 0, end)
            self.assertGreaterEqual(start, 0)
            contract = original[start:end]
            wrong_projection = (
                "sequence_outcome" if projection == "reconstruction_outcome"
                else "reconstruction_outcome"
            )
            for before, after in (
                (f"ensures {projection}(result)", f"ensures {projection}(unrelated)"),
                (f"ensures {projection}(result)", "ensures unlisted_projection(result)"),
                (f"ensures {projection}(result)", f"ensures {wrong_projection}(result)"),
                ("verus_spec(result =>", "verus_spec(unrelated =>"),
                (f"ensures {projection}(result)", "ensures result.is_ok()"),
                (f"== crate::specs::{model}(", "== crate::specs::unlisted_model("),
            ):
                self.assertIn(before, contract)
                modified = original[:start] + contract.replace(before, after) + original[end:]

                def altered(path, *args, **kwargs):
                    return modified if path == source else read_text(path, *args, **kwargs)

                with self.subTest(function=function, after=after), \
                        mock.patch.object(Path, "read_text", altered), \
                        self.assertRaisesRegex(ValueError, "lacks executable Verus contract"):
                    framework.validate_normative_coverage(coverage)

    def test_inventory_requires_complete_normalized_plane_bound_result(self):
        coverage = json.loads((framework.ROOT / "normative-coverage.json").read_text())
        coverage["requirements"] = [
            item for item in coverage["requirements"]
            if item["id"] == "part1-complete-normalized-component-plane"
        ]
        self.assertEqual(framework.validate_normative_coverage(coverage)["status"], "passed")
        read_text = Path.read_text
        for before, after in (
            ("ensures normalized_plane_result(result)", "ensures normalized_plane_result(unrelated)"),
            ("ensures normalized_plane_result(result)", "ensures unlisted_projection(result)"),
            ("ensures normalized_plane_result(result)", "ensures converted_image_result(result)"),
            ("ensures normalized_plane_result(result)", "ensures result.is_ok()"),
            ("== crate::specs::color_normalize_component(", "== crate::specs::unlisted_model("),
        ):
            def altered(path, *args, **kwargs):
                text = read_text(path, *args, **kwargs)
                return text.replace(before, after) if path == framework.ROOT / "src/color.rs" else text
            with self.subTest(after=after), mock.patch.object(Path, "read_text", altered), \
                    self.assertRaisesRegex(ValueError, "lacks executable Verus contract"):
                framework.validate_normative_coverage(coverage)

    def test_inventory_requires_complete_channel_layout_bound_result(self):
        coverage = json.loads((framework.ROOT / "normative-coverage.json").read_text())
        coverage["requirements"] = [
            item for item in coverage["requirements"]
            if item["id"] == "part2-complete-component-channel-layout"
        ]
        self.assertEqual(framework.validate_normative_coverage(coverage)["status"], "passed")
        read_text = Path.read_text
        for before, after in (
            ("ensures channel_layout_result(result)", "ensures channel_layout_result(unrelated)"),
            ("ensures channel_layout_result(result)", "ensures unlisted_projection(result)"),
            ("ensures channel_layout_result(result)", "ensures normalized_plane_result(result)"),
            ("ensures channel_layout_result(result)", "ensures result.is_ok()"),
            ("== crate::specs::color_channel_layout(", "== crate::specs::unlisted_model("),
        ):
            def altered(path, *args, **kwargs):
                text = read_text(path, *args, **kwargs)
                return text.replace(before, after) if path == framework.ROOT / "src/color.rs" else text
            with self.subTest(after=after), mock.patch.object(Path, "read_text", altered), \
                    self.assertRaisesRegex(ValueError, "lacks executable Verus contract"):
                framework.validate_normative_coverage(coverage)

    def test_inventory_binds_complete_ht_parser_to_its_exact_outcome_and_model(self):
        coverage = json.loads((framework.ROOT / "normative-coverage.json").read_text())
        coverage["requirements"] = [
            item for item in coverage["requirements"]
            if item["id"] == "part15-cleanup-partition"
        ]
        self.assertEqual(len(coverage["requirements"]), 1)
        coverage["requirements"][0]["contract_refs"] = [
            {"path": "src/tier1.rs", "symbol": "parse_ht_block_segments"}
        ]
        self.assertEqual(framework.validate_normative_coverage(coverage)["status"], "passed")
        source = framework.ROOT / "src/tier1.rs"
        read_text = Path.read_text
        original = source.read_text()
        end = re.search(r"\bfn\s+parse_ht_block_segments\b", original).start()
        start = original.rfind("#[cfg_attr", 0, end)
        contract = original[start:end]
        for before, after in (
            ("ensures ht_block_segments_outcome(result)", "ensures ht_block_segments_outcome(unrelated)"),
            ("ensures ht_block_segments_outcome(result)", "ensures unlisted_projection(result)"),
            ("ensures ht_block_segments_outcome(result)", "ensures raw_reader_outcome(result)"),
            ("ensures ht_block_segments_outcome(result)", "ensures result.is_ok()"),
            ("== crate::specs::ht_parse_block_segments(", "== crate::specs::unlisted_model("),
            ("== crate::specs::ht_parse_block_segments(", "== crate::specs::ht_scup("),
            ("== crate::specs::ht_parse_block_segments(", "<= crate::specs::ht_parse_block_segments("),
        ):
            self.assertIn(before, contract)
            modified = original[:start] + contract.replace(before, after) + original[end:]

            def altered(path, *args, **kwargs):
                return modified if path == source else read_text(path, *args, **kwargs)

            with self.subTest(after=after), mock.patch.object(Path, "read_text", altered), \
                    self.assertRaisesRegex(ValueError, "lacks executable Verus contract"):
                framework.validate_normative_coverage(coverage)

    def test_inventory_binds_complete_crop_to_exact_full_result_models(self):
        original_coverage = json.loads((framework.ROOT / "normative-coverage.json").read_text())
        source = framework.ROOT / "src/color.rs"
        original = source.read_text()
        read_text = Path.read_text
        for symbol, projection in (
            ("ConvertedImage::crop", "converted_image_result(result)"),
            ("crop_plane", "normalized_plane_result(result)"),
            ("crop_sample", "result"),
        ):
            coverage = copy.deepcopy(original_coverage)
            coverage["requirements"] = [
                item for item in coverage["requirements"]
                if item["id"] == "part1-complete-converted-selection-crop"
            ]
            self.assertEqual(len(coverage["requirements"]), 1)
            coverage["requirements"][0]["contract_refs"] = [{"path": "src/color.rs", "symbol": symbol}]
            self.assertEqual(framework.validate_normative_coverage(coverage)["status"], "passed")
            short = symbol.rsplit("::", 1)[-1]
            end = re.search(rf"\bfn\s+{re.escape(short)}\b", original).start()
            start = original.rfind("#[cfg_attr", 0, end)
            contract = original[start:end]
            unbound = "unrelated" if projection == "result" else projection.replace("(result)", "(unrelated)")
            for before, after in (
                (f"ensures {projection}", f"ensures {unbound}"),
                (f"ensures {projection}", "ensures unlisted_projection(result)"),
                (f"ensures {projection}", "ensures result.is_ok()"),
                ("== crate::specs::", "<= crate::specs::"),
            ):
                self.assertIn(before, contract)
                modified = original[:start] + contract.replace(before, after) + original[end:]

                def altered(path, *args, **kwargs):
                    return modified if path == source else read_text(path, *args, **kwargs)

                with self.subTest(symbol=symbol, after=after), mock.patch.object(Path, "read_text", altered), \
                        self.assertRaisesRegex(ValueError, "lacks executable Verus contract"):
                    framework.validate_normative_coverage(coverage)
            wrong_model = "color_crop_plane" if short == "crop" else "color_crop_image"
            modified_contract = re.sub(r"== crate::specs::\w+\(", f"== crate::specs::{wrong_model}(", contract, count=1)
            self.assertNotEqual(modified_contract, contract)
            modified = original[:start] + modified_contract + original[end:]
            with mock.patch.object(Path, "read_text",
                                   lambda path, *args, **kwargs:
                                   modified if path == source else read_text(path, *args, **kwargs)), \
                    self.assertRaisesRegex(ValueError, "lacks executable Verus contract"):
                framework.validate_normative_coverage(coverage)

    def test_inventory_binds_each_complete_color_converter_result_to_its_exact_model(self):
        original_coverage = json.loads((framework.ROOT / "normative-coverage.json").read_text())
        source = framework.ROOT / "src/color.rs"
        original = source.read_text()
        read_text = Path.read_text
        for function, projection in (
            ("convert_jp2_color", "converted_image_result(result)"),
            ("convert_color_channel", "converted_channel_result(result)"),
            ("convert_alpha_channel", "converted_channel_result(result)"),
            ("convert_sycc_pixel", "result"),
            ("validate_components", "result"),
            ("validate_component_index", "result"),
            ("validate_component", "result"),
            ("validate_output_size", "result"),
        ):
            coverage = copy.deepcopy(original_coverage)
            coverage["requirements"] = [
                item for item in coverage["requirements"]
                if item["id"] == "part1-complete-typed-color-conversion"
            ]
            self.assertEqual(len(coverage["requirements"]), 1)
            coverage["requirements"][0]["contract_refs"] = [
                {"path": "src/color.rs", "symbol": function}
            ]
            self.assertEqual(framework.validate_normative_coverage(coverage)["status"], "passed")
            end = re.search(rf"\bfn\s+{re.escape(function)}\b", original).start()
            start = original.rfind("#[cfg_attr", 0, end)
            contract = original[start:end]
            wrong_model = "color_convert_channel" if function == "convert_jp2_color" else "color_convert_jp2"
            unbound = "unrelated" if projection == "result" else projection.replace("(result)", "(unrelated)")
            for before, after in (
                (f"ensures {projection}", f"ensures {unbound}"),
                (f"ensures {projection}", "ensures unlisted_projection(result)"),
                (f"ensures {projection}", "ensures normalized_plane_result(result)"),
                (f"ensures {projection}", "ensures result.is_ok()"),
                ("== crate::specs::", "<= crate::specs::"),
            ):
                self.assertIn(before, contract)
                modified = original[:start] + contract.replace(before, after) + original[end:]

                def altered(path, *args, **kwargs):
                    return modified if path == source else read_text(path, *args, **kwargs)

                with self.subTest(function=function, after=after), \
                        mock.patch.object(Path, "read_text", altered), \
                        self.assertRaisesRegex(ValueError, "lacks executable Verus contract"):
                    framework.validate_normative_coverage(coverage)
            modified_contract = re.sub(
                r"== crate::specs::\w+\(", f"== crate::specs::{wrong_model}(", contract, count=1,
            )
            self.assertNotEqual(modified_contract, contract)
            modified = original[:start] + modified_contract + original[end:]
            with self.subTest(function=function, wrong_listed_model=wrong_model), \
                    mock.patch.object(Path, "read_text",
                                      lambda path, *args, **kwargs:
                                      modified if path == source else read_text(path, *args, **kwargs)), \
                    self.assertRaisesRegex(ValueError, "lacks executable Verus contract"):
                framework.validate_normative_coverage(coverage)

    def test_inventory_requires_all_mutable_outputs_in_planar_state_relation(self):
        coverage = json.loads((framework.ROOT / "normative-coverage.json").read_text())
        coverage["requirements"] = [
            item for item in coverage["requirements"]
            if item["id"] == "part2-jpx-composition-rendering"
        ]
        coverage["requirements"][0]["contract_refs"] = [
            {"path": "src/refinement.rs", "symbol": symbol}
            for symbol in ("scatter_composition_pixel", "compose_planar_pixel")
        ]
        self.assertEqual(framework.validate_normative_coverage(coverage)["status"], "passed")
        read_text = Path.read_text
        for before, after in (
            ("verus_spec(composed =>", "verus_spec(unrelated =>"),
            ("ensures (composed,", "ensures (true,"),
            ("part2_composition_planes(final(output_colors)@), final(output_alpha)@)",
             "part2_composition_planes(final(output_colors)@))"),
            ("part2_composition_planes(final(output_colors)@), final(output_alpha)@)",
             "part2_composition_planes(final(output_colors)@), final(output_colors)@)"),
            ("part2_composition_planes(final(output_colors)@)",
             "part2_composition_planes(old(output_colors)@)"),
            ("final(output_alpha)@", "old(output_alpha)@"),
            ("part2_composition_planes(final(output_colors)@)",
             "unlisted_projection(final(output_colors)@)"),
            ("== crate::specs::part2_composition_pixel_transition",
             "== crate::specs::unlisted_model"),
        ):
            def altered(path, *args, **kwargs):
                text = read_text(path, *args, **kwargs)
                return text.replace(before, after) if path == framework.ROOT / "src/refinement.rs" else text
            with self.subTest(after=after), mock.patch.object(Path, "read_text", altered), \
                    self.assertRaisesRegex(ValueError, "lacks executable Verus contract"):
                framework.validate_normative_coverage(coverage)

    def test_inventory_requires_bound_result_and_complete_mutable_reader_view(self):
        coverage = json.loads((framework.ROOT / "normative-coverage.json").read_text())
        coverage["requirements"] = [
            item for item in coverage["requirements"]
            if item["id"] == "part1-raw-bitstream-state"
        ]
        self.assertEqual(framework.validate_normative_coverage(coverage)["status"], "passed")
        read_text = Path.read_text
        for before, after in (
            ("ensures raw_reader_outcome(result)", "ensures raw_reader_outcome(unrelated)"),
            ("ensures raw_reader_outcome(result)", "ensures unlisted_projection(result)"),
            ("ensures (result, raw_bit_reader_view(final(self)))",
             "ensures (true, raw_bit_reader_view(final(self)))"),
            ("ensures (result, raw_bit_reader_view(final(self)))",
             "ensures (result, raw_bit_reader_view(old(self)))"),
            ("ensures (result, raw_bit_reader_view(final(self)))",
             "ensures (result, raw_bit_reader_view(final(self)).bits_remaining)"),
            ("ensures (result, raw_bit_reader_view(final(self)))",
             "ensures (result, unlisted_projection(final(self)))"),
            ("== crate::specs::part1_raw_reader_read(", "== crate::specs::unlisted_model("),
        ):
            def altered(path, *args, **kwargs):
                text = read_text(path, *args, **kwargs)
                return text.replace(before, after) if path == framework.ROOT / "src/tier1.rs" else text
            with self.subTest(after=after), mock.patch.object(Path, "read_text", altered), \
                    self.assertRaisesRegex(ValueError, "lacks executable Verus contract"):
                framework.validate_normative_coverage(coverage)

    def test_inventory_resolves_duplicate_methods_inside_the_requested_impl(self):
        text = """impl Legacy {
    fn read_bit(&mut self) {
        legacy_marker();
    }
}
impl<'a> HtMelBitReader<'a> {
    fn read_bit(&mut self) {
        mel_marker();
    }
}
impl HtReverseVlcReader {
    fn read_bit(&mut self) {
        vlc_marker();
    }
}
"""
        self.assertTrue(framework._coverage_same_file_call(text, "Legacy::read_bit", "legacy_marker"))
        self.assertFalse(framework._coverage_same_file_call(text, "Legacy::read_bit", "mel_marker"))
        self.assertTrue(framework._coverage_same_file_call(text, "HtMelBitReader::read_bit", "mel_marker"))
        self.assertFalse(framework._coverage_same_file_call(text, "HtMelBitReader::read_bit", "vlc_marker"))
        self.assertTrue(framework._coverage_same_file_call(text, "HtReverseVlcReader::read_bit", "vlc_marker"))
        self.assertIsNone(framework._coverage_function_header(text, "Unknown::read_bit"))

    def test_inventory_requires_function_specific_complete_ht_reader_relations(self):
        original = json.loads((framework.ROOT / "normative-coverage.json").read_text())
        original["requirements"] = [
            item for item in original["requirements"]
            if item["id"] in (
                "part15-cleanup-reader-single-bit-state", "part15-directional-word-event-state",
                "part15-uvlc-quad-word-state",
            )
        ]
        self.assertEqual(framework.validate_normative_coverage(original)["status"], "passed")
        source_path = framework.ROOT / "src/tier1.rs"
        read_text = Path.read_text
        source = read_text(source_path)
        entries = [
            ("ht_mel_from_input", "ht_mel_outcome", "ht_mel_initial", False),
            ("ht_reverse_vlc_from_input", "ht_reverse_vlc_outcome", "ht_reverse_vlc_initial", False),
            ("ht_cleanup_readers_from_inputs", "ht_cleanup_readers_outcome", "ht_cleanup_readers_initial", False),
            ("HtMelBitReader::load_byte", "ht_mel_view", "ht_mel_load", True),
            ("HtMelBitReader::read_bit", "ht_mel_view", "ht_mel_read", True),
            ("HtReverseVlcReader::load_previous_byte", "ht_reverse_vlc_view", "ht_reverse_vlc_load", True),
            ("HtReverseVlcReader::read_bit", "ht_reverse_vlc_view", "ht_reverse_vlc_read", True),
            ("HtCleanupReaders::tick", "ht_cleanup_readers_view", "ht_cleanup_reader_tick", True),
            ("HtCleanupReaders::read_mel_bit", "ht_cleanup_readers_view", "ht_cleanup_reader_read_mel", True),
            ("HtCleanupReaders::read_vlc_bit", "ht_cleanup_readers_view", "ht_cleanup_reader_read_vlc", True),
            ("ht_magsgn_from_input", "ht_magsgn_outcome", "ht_magsgn_initial", False),
            ("ht_forward_refinement_from_input", "ht_forward_refinement_outcome", "ht_forward_refinement_initial", False),
            ("ht_reverse_refinement_from_input", "ht_reverse_refinement_outcome", "ht_reverse_refinement_initial", False),
            ("ht_refinement_readers_from_inputs", "ht_refinement_readers_outcome", "ht_refinement_readers_initial", False),
            ("HtForwardMagSgnReader::load_byte", "ht_magsgn_view", "ht_magsgn_load", True),
            ("HtForwardMagSgnReader::read_bit", "ht_magsgn_view", "ht_magsgn_read", True),
            ("HtForwardMagSgnReader::read_bits", "ht_magsgn_view", "ht_magsgn_read_bits", True),
            ("HtForwardMagSgnReader::finish", "ht_magsgn_view", "ht_magsgn_finish", True),
            ("HtForwardRefinementReader::load_byte", "ht_forward_refinement_view", "ht_forward_refinement_load", True),
            ("HtForwardRefinementReader::read_positioned_bit", "ht_forward_refinement_view", "ht_forward_refinement_positioned", True),
            ("HtForwardRefinementReader::read_bit", "ht_forward_refinement_view", "ht_forward_refinement_read", True),
            ("HtReverseRefinementReader::load_previous_byte", "ht_reverse_refinement_view", "ht_reverse_refinement_load", True),
            ("HtReverseRefinementReader::read_positioned_bit", "ht_reverse_refinement_view", "ht_reverse_refinement_positioned", True),
            ("HtReverseRefinementReader::read_bit", "ht_reverse_refinement_view", "ht_reverse_refinement_read", True),
            ("HtRefinementReaders::claim", "ht_refinement_readers_view", "ht_refinement_claim", True),
            ("HtRefinementReaders::read_forward", "ht_refinement_readers_view", "ht_refinement_read_forward", True),
            ("HtRefinementReaders::read_reverse", "ht_refinement_readers_view", "ht_refinement_read_reverse", True),
            ("HtCleanupReaders::advance_vlc", "ht_cleanup_readers_view", "ht_cleanup_advance_vlc", True),
            ("HtCleanupReaders::mel_event", "ht_cleanup_readers_view", "ht_cleanup_mel_event", True),
            ("HtCleanupReaders::read_prefix", "ht_cleanup_readers_view", "ht_uvlc_prefix", True),
            ("HtCleanupReaders::read_suffix", "ht_cleanup_readers_view", "ht_uvlc_suffix", True),
            ("HtCleanupReaders::prefix_value", "ht_cleanup_readers_view", "ht_uvlc_prefix_value", True),
            ("HtCleanupReaders::decode_initial_u", "ht_cleanup_readers_view", "ht_uvlc_initial", True),
            ("HtCleanupReaders::decode_non_initial_u", "ht_cleanup_readers_view", "ht_uvlc_non_initial", True),
            ("HtCleanupReaders::decode_quad_word", "ht_cleanup_readers_view", "ht_cleanup_quad_word", True),
        ]
        for symbol, projection, model, stateful in entries:
            coverage = copy.deepcopy(original)
            coverage["requirements"] = [
                item for item in coverage["requirements"]
                if any(ref["symbol"] == model for ref in item["spec_refs"])
            ][:1]
            coverage["requirements"][0]["contract_refs"] = [{"path": "src/tier1.rs", "symbol": symbol}]
            if symbol in (
                "HtForwardRefinementReader::read_bit", "HtReverseRefinementReader::read_bit",
                "HtForwardMagSgnReader::read_bits",
            ):
                # These public adapters have real API tests, not internal decoder callers.
                coverage["requirements"][0]["implementation_refs"] = [{
                    "path": "src/tier1.rs",
                    "symbol": (
                        "ht_magsgn_words_have_exact_fill_and_physical_state"
                        if symbol == "HtForwardMagSgnReader::read_bits"
                        else "ht_refinement_counter_and_sum_overflow_preserve_ordered_cursor_updates"
                    ),
                }]
            self.assertEqual(framework.validate_normative_coverage(coverage)["status"], "passed")
            header = framework._coverage_function_header(source, symbol)
            self.assertIsNotNone(header)
            signature = source.find("fn", header.start(), header.end())
            start = source.rfind("#[cfg_attr", 0, signature)
            contract = source[start:signature]
            binder = re.search(r"\bverus_spec\(\s*(\w+)\s*=>", contract)[1]
            call = re.escape(projection) + r"\(\s*" + (
                r"final\(\s*self\s*\)" if stateful else re.escape(binder)
            ) + r"\s*\)"
            wrong_view = "ht_reverse_vlc_view" if projection == "ht_mel_view" else "ht_mel_view"
            wrong_model = next(
                ref["symbol"] for ref in coverage["requirements"][0]["spec_refs"]
                if ref["symbol"] != model
            )
            replacements = [
                re.sub(r"\bverus_spec\(\s*\w+\s*=>", "verus_spec(unrelated =>", contract, count=1),
                re.sub(call, "unlisted_projection(" + ("final(self)" if stateful else binder) + ")", contract, count=1),
                contract.replace("crate::specs::" + model + "(", "crate::specs::" + wrong_model + "(", 1),
            ]
            if stateful:
                replacements.extend((
                    re.sub(call, projection + "(old(self))", contract, count=1),
                    re.sub(call, projection + "(final(self)).work_units", contract, count=1),
                    re.sub(call, wrong_view + "(final(self))", contract, count=1),
                    contract.replace("old(self)", "self", 1),
                ))
            else:
                replacements.append(re.sub(call, projection + "(unrelated)", contract, count=1))
            for replacement in replacements:
                self.assertNotEqual(replacement, contract)

                def altered(path, *args, **kwargs):
                    text = read_text(path, *args, **kwargs)
                    return text[:start] + replacement + text[signature:] if path == source_path else text

                with self.subTest(symbol=symbol, replacement=replacement[:120]), \
                        mock.patch.object(Path, "read_text", altered), \
                        self.assertRaisesRegex(ValueError, "lacks executable Verus contract"):
                    framework.validate_normative_coverage(coverage)
        wrong = copy.deepcopy(original)
        wrong["requirements"][0]["contract_refs"] = [{
            "path": "src/tier1.rs", "symbol": "Unknown::read_bit",
        }]
        with self.assertRaisesRegex(ValueError, "missing linked contract function"):
            framework.validate_normative_coverage(wrong)

    def test_inventory_checks_success_guard_and_all_layer_outputs(self):
        coverage = json.loads((framework.ROOT / "normative-coverage.json").read_text())
        coverage["requirements"] = [
            item for item in coverage["requirements"]
            if item["id"] == "part2-jpx-composition-rendering"
        ]
        coverage["requirements"][0]["contract_refs"] = [{
            "path": "src/jpx.rs", "symbol": "composite_placed_layer",
        }]
        self.assertEqual(framework.validate_normative_coverage(coverage)["status"], "passed")
        read_text = Path.read_text
        for before, after in (
            ("verus_spec(result =>", "verus_spec(unrelated =>"),
            ("ensures result.is_ok() ==>", "ensures unrelated.is_ok() ==>"),
            ("ensures result.is_ok() ==>", "ensures"),
            ("(result.is_ok(), crate::specs::", "(true, crate::specs::"),
            (", *final(work_units) as nat)", ")"),
            ("*final(work_units) as nat", "*old(work_units) as nat"),
            ("*final(work_units) as nat", "final(output_alpha)@"),
            ("== crate::specs::part2_composition_layer_success", "== crate::specs::unlisted_model"),
        ):
            def altered(path, *args, **kwargs):
                text = read_text(path, *args, **kwargs)
                return text.replace(before, after) if path == framework.ROOT / "src/jpx.rs" else text
            with self.subTest(after=after), mock.patch.object(Path, "read_text", altered), \
                    self.assertRaisesRegex(ValueError, "lacks executable Verus contract"):
                framework.validate_normative_coverage(coverage)

    def test_inventory_same_file_usage_requires_another_listed_function_body(self):
        coverage = json.loads((framework.ROOT / "normative-coverage.json").read_text())
        coverage["requirements"] = [
            item for item in coverage["requirements"]
            if item["id"] == "part2-jpx-composition-rendering"
        ]
        entry = coverage["requirements"][0]
        entry["contract_refs"] = [{"path": "src/jpx.rs", "symbol": "composite_placed_layer"}]
        entry["implementation_refs"] = [{"path": "src/jpx.rs", "symbol": "render_decoded_layers"}]
        self.assertEqual(framework.validate_normative_coverage(coverage)["status"], "passed")
        for symbol in ("index_of", "render_jpx_frame", "composite_placed_layer"):
            entry["implementation_refs"] = [{"path": "src/jpx.rs", "symbol": symbol}]
            with self.subTest(symbol=symbol), \
                    self.assertRaisesRegex(ValueError, "not used by claimed implementation"):
                framework.validate_normative_coverage(coverage)

    def test_inventory_checks_typed_result_model_equalities(self):
        coverage = json.loads((framework.ROOT / "normative-coverage.json").read_text())
        coverage["requirements"] = [
            item for item in coverage["requirements"]
            if item["id"] == "part2-jpx-composition-rendering"
        ]
        coverage["requirements"][0]["contract_refs"] = [{
            "path": "src/refinement.rs", "symbol": symbol,
        } for symbol in ("composition_clip", "composition_pixel_index", "composition_mapped_offset")]
        self.assertEqual(framework.validate_normative_coverage(coverage)["status"], "passed")
        read_text = Path.read_text
        for before, after in (
            ("verus_spec(index =>", "verus_spec(unrelated =>"),
            ("ensures index ==", "ensures index.is_some() =="),
            ("ensures index ==", "ensures None =="),
            ("ensures clipped ==", "ensures clipped >="),
            ("ensures mapped as nat ==", "requires mapped as nat =="),
            ("== crate::specs::part2_composition_pixel_index", "== crate::specs::unlisted_model"),
        ):
            def altered(path, *args, **kwargs):
                text = read_text(path, *args, **kwargs)
                return text.replace(before, after) if path == framework.ROOT / "src/refinement.rs" else text
            with self.subTest(after=after), mock.patch.object(Path, "read_text", altered), \
                    self.assertRaisesRegex(ValueError, "lacks executable Verus contract"):
                framework.validate_normative_coverage(coverage)

    def test_build_subject_is_locked(self):
        executable = self.work / "built-subject"
        executable.touch()
        with mock.patch.object(framework, "resolve_subject", return_value=executable), mock.patch.object(framework.subprocess, "run") as run:
            self.assertEqual(framework.build_subject(), executable)
        self.assertIn("--locked", run.call_args.args[0])

    def test_run_outputs_are_redirectable_and_schemas_accept_new_results(self):
        subject = self.subject()
        result = framework.run_validation(subject, include_external=False, output_dir=self.work)
        self.assertEqual(result["summary"]["status"], "passed")
        self.assertEqual(result["summary"]["external_oracles_available"], 0)
        adjudications = json.loads((self.work / "oracle-adjudications.json").read_text())
        self.assertEqual(adjudications["status"], "skipped")
        self.assertRegex(result["provenance"]["git_commit"], r"^[a-f0-9]{40,64}$")
        self.assertIsInstance(result["provenance"]["git_dirty"], bool)
        self.assertTrue(result["provenance"]["tool_hashes"])
        self.assertTrue((self.work / result["$schema"]).is_file())
        budget = framework.run_budget(subject, output_dir=self.work)
        self.assertEqual(budget["summary"]["status"], "passed")
        self.assertEqual(framework.validate_evidence_schemas(self.work)["status"], "passed")
        self.assertFalse((framework.VALIDATION / "results/validation-results.json").exists())

    def test_default_output_is_ignored_and_tracked_overwrites_are_refused(self):
        self.assertEqual(framework.RESULTS_DIR, framework.ROOT / "target/validation")
        path = framework.report_path(None, "validation-results.json")
        result = subprocess.run(["git", "check-ignore", str(path)], cwd=framework.ROOT, capture_output=True)
        self.assertEqual(result.returncode, 0)
        with self.assertRaisesRegex(ValueError, "tracked"):
            framework.report_path(framework.ROOT, "Cargo.toml")

    def test_missing_unconfigured_oracle_is_skipped(self):
        with mock.patch.dict(os.environ, {}, clear=True), mock.patch.object(framework.shutil, "which", return_value=None):
            result = framework.run_validation(self.subject(), oracle_config=self.oracle_config(), output_dir=self.work)
        self.assertEqual(result["summary"]["status"], "passed")
        self.assertEqual(result["oracles"][1]["status"], "skipped")
        self.assertEqual(result["summary"]["external_oracles_skipped"], 1)

    def test_explicit_missing_oracle_is_failed_not_skipped(self):
        with mock.patch.dict(os.environ, {"OPENJPEG_DECODER": str(self.work / "missing")}, clear=True):
            result = framework.run_validation(self.subject(), oracle_config=self.oracle_config(), output_dir=self.work)
        self.assertEqual(result["summary"]["status"], "failed")
        self.assertEqual(result["oracles"][1]["status"], "failed")
        self.assertEqual(result["summary"]["oracle_errors"], 1)

    def test_invalid_oracle_output_is_failed_not_a_rejection_or_skip(self):
        oracle = self.work / "oracle"
        oracle.write_text(f"#!{sys.executable}\nraise SystemExit(0)\n")
        oracle.chmod(0o700)
        with mock.patch.dict(os.environ, {"OPENJPEG_DECODER": str(oracle)}, clear=True):
            result = framework.run_validation(self.subject(), oracle_config=self.oracle_config(), output_dir=self.work)
        self.assertEqual(result["summary"]["status"], "failed")
        self.assertGreater(result["summary"]["oracle_errors"], 0)
        self.assertTrue(any(
            record["outcomes"].get("openjpeg", {}).get("execution_failed")
            for record in result["records"]
        ))
        adjudications = json.loads((self.work / "oracle-adjudications.json").read_text())
        self.assertEqual(adjudications["status"], "blocked")

    def test_oracle_timeout_is_failure_not_skip(self):
        subject = self.subject()
        oracle = self.work / "oracle"
        shutil.copyfile(subject, oracle)
        oracle.chmod(0o700)
        original_run = framework._run_process

        def timeout_external(command, timeout):
            if command[0] == str(oracle):
                raise subprocess.TimeoutExpired(command, timeout)
            return original_run(command, timeout)

        with mock.patch.dict(os.environ, {"OPENJPEG_DECODER": str(oracle)}, clear=True), mock.patch.object(framework, "_run_process", side_effect=timeout_external):
            result = framework.run_validation(subject, oracle_config=self.oracle_config(), output_dir=self.work)
        self.assertEqual(result["summary"]["status"], "failed")
        self.assertGreater(result["summary"]["oracle_errors"], 0)

    def test_subject_process_failure_does_not_pass_negative_case(self):
        parsed = framework.parse_subject_output(b"", b"crash", -9)
        self.assertTrue(parsed["execution_failed"])
        self.assertEqual(parsed["category"], "process-error")

    def test_disagreements_are_retained_and_external_adjudications_not_rewritten(self):
        oracle = self.work / "oracle"
        oracle.write_text(f"#!{sys.executable}\nraise SystemExit(1)\n")
        oracle.chmod(0o700)
        with mock.patch.dict(os.environ, {"OPENJPEG_DECODER": str(oracle)}, clear=True):
            result = framework.run_validation(self.subject(), oracle_config=self.oracle_config(), output_dir=self.work)
            self.assertEqual(result["summary"]["status"], "blocked")
            self.assertTrue(result["disagreements"])
            external = self.work / "external-adjudications.json"
            adjudications = json.loads((self.work / "oracle-adjudications.json").read_text())
            for record in adjudications["records"]:
                record.update(status="adjudicated", decision="retain observation", rationale="synthetic fixture decision", adjudicator="unit test", adjudicated_at="2026-10-19T00:00:00Z")
            external.write_text(json.dumps(adjudications))
            before = external.read_bytes()
            result = framework.run_validation(self.subject(), oracle_config=self.oracle_config(), output_dir=self.work, adjudication_file=external)
        self.assertEqual(result["summary"]["status"], "passed")
        self.assertTrue(result["disagreements"])
        self.assertEqual(external.read_bytes(), before)
        framework.validate_evidence_schemas(self.work)

    def test_schema_check_does_not_require_existing_results(self):
        result = framework.validate_evidence_schemas(self.work)
        self.assertEqual(result["status"], "passed")
        self.assertEqual(result["reports"], [])

    def test_clean_clone_without_private_root_evidence_runs_all(self):
        clone = self.work / "source"
        clone.mkdir()
        for directory in ("src", "specs", "proofs", "examples", "validation"):
            shutil.copytree(framework.ROOT / directory, clone / directory, ignore=shutil.ignore_patterns("__pycache__", "results"))
        for name in ("Cargo.toml", "Cargo.lock", "rust-toolchain.toml", "normative-coverage.json", "normative-coverage.schema.json"):
            shutil.copyfile(framework.ROOT / name, clone / name)
        root_json = {path.name for path in clone.glob("*.json")}
        self.assertEqual(root_json, {"normative-coverage.json", "normative-coverage.schema.json"})
        source_hashes = {
            path: framework.sha256_file(path)
            for path in clone.rglob("*") if path.is_file()
        }
        completed = subprocess.run(
            [sys.executable, str(clone / "validation/run-validation.py"), "all", "--no-external", "--subject", str(self.subject())],
            cwd=clone, capture_output=True, text=True,
        )
        self.assertEqual(completed.returncode, 0, completed.stderr)
        self.assertTrue((clone / "target/validation/validation-results.json").is_file())
        self.assertFalse((clone / "validation/results").exists())
        self.assertEqual(source_hashes, {path: framework.sha256_file(path) for path in source_hashes})

    def test_budget_output_is_bounded_and_limit_failure_cannot_pass(self):
        subject = self.work / "noisy-subject"
        subject.write_text(f"#!{sys.executable}\nprint('x' * 100000)\n")
        subject.chmod(0o700)
        result = framework.run_budget(subject, max_output_bytes=512, output_dir=self.work)
        self.assertEqual(result["summary"]["status"], "failed")
        self.assertTrue(any("output budget" in failure for failure in result["summary"]["failures"]))

    def test_official_unconfigured_is_explicitly_blocked(self):
        with mock.patch.dict(os.environ, {}, clear=True):
            result = framework.run_official_vectors(output_dir=self.work)
        self.assertEqual(result["status"], "blocked")
        self.assertEqual(result["acquisition"]["access_status"], "not-configured")
        self.assertEqual(result["records"], [])
        framework.validate_evidence_schemas(self.work)

    def test_official_manifest_execution_uses_hashes_and_redacts_private_identifiers(self):
        path, manifest = self.external_manifest()
        result = framework.run_official_vectors(self.subject(), self.work, path, output_dir=self.work)
        self.assertEqual(result["status"], "passed")
        self.assertEqual(result["source_url"], manifest["suite"]["source_url"])
        self.assertFalse(result["license"]["redistribution_permitted"])
        serialized = json.dumps(result)
        self.assertNotIn("private-fixture-id", serialized)
        self.assertNotIn(str(self.work), serialized)
        framework.validate_evidence_schemas(self.work)

    def test_malformed_official_manifests_fail_before_execution(self):
        path, original = self.external_manifest()
        mutations = [
            lambda m: m["suite"].pop("license"),
            lambda m: m["suite"]["license"].update(redistribution_permitted=True),
            lambda m: m["suite"].update(acquired_at="not-a-date"),
            lambda m: m["entries"][0].update(path="../outside.j2k"),
            lambda m: m["entries"][0].update(sha256="invalid"),
            lambda m: m["entries"][0].update(expected={"accepted": True}),
            lambda m: m["entries"].append(copy.deepcopy(m["entries"][0])),
        ]
        for mutate in mutations:
            manifest = copy.deepcopy(original)
            mutate(manifest)
            path.write_text(json.dumps(manifest))
            with self.subTest(mutation=mutate), mock.patch.object(framework, "_run_process_bounded") as run:
                with self.assertRaises(ValueError):
                    framework.run_official_vectors(self.subject(), self.work, path, output_dir=self.work)
                run.assert_not_called()
        path.write_text("{")
        with self.assertRaisesRegex(ValueError, "invalid external official vector manifest"):
            framework.run_official_vectors(self.subject(), self.work, path, output_dir=self.work)

    def test_official_hash_mismatch_and_escaped_symlink_fail(self):
        path, manifest = self.external_manifest()
        (self.work / "vector.j2k").write_bytes(b"wrong")
        result = framework.run_official_vectors(self.subject(), self.work, path, output_dir=self.work)
        self.assertEqual(result["status"], "failed")
        outside = self.work.parent / f"outside-{uuid.uuid4().hex}.j2k"
        outside.write_bytes(b"outside fixture")
        self.addCleanup(outside.unlink)
        (self.work / "vector.j2k").unlink()
        (self.work / "vector.j2k").symlink_to(outside)
        with self.assertRaisesRegex(ValueError, "escapes"):
            framework.run_official_vectors(self.subject(), self.work, path, output_dir=self.work)

    def test_official_invalid_archive_never_executes_a_vector(self):
        path, _ = self.external_manifest()
        (self.work / "suite.bin").write_bytes(b"wrong archive")
        with mock.patch.object(framework, "_run_process_bounded") as run:
            result = framework.run_official_vectors(self.subject(), self.work, path, output_dir=self.work)
            run.assert_not_called()
        self.assertEqual(result["status"], "failed")
        self.assertEqual(result["summary"]["executed"], 0)


if __name__ == "__main__":
    unittest.main()
