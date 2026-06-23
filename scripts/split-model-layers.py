#!/usr/bin/env python3
"""Split a GGUF model into standalone per-stage files for pipeline v3 (activation passing).

Each stage file is a complete GGUF (split_count=1) with only its layer tensors.
Stage 1+ layers are renumbered to blk.0..blk.(n-1) so llama.cpp can load them alone.
"""
from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path.home() / ".youai" / "llama.cpp" / "gguf-py"))
from gguf import GGUFReader, GGUFWriter, GGUFValueType, Keys, GGMLQuantizationType  # noqa: E402

BLK_RE = re.compile(r"^blk\.(\d+)\.(.+)$")


def layer_index(name: str) -> int | None:
    match = BLK_RE.match(name)
    if not match:
        return None
    return int(match.group(1))


def rename_block(name: str, offset: int) -> str:
    match = BLK_RE.match(name)
    if not match:
        return name
    return f"blk.{int(match.group(1)) - offset}.{match.group(2)}"


def copy_metadata(reader: GGUFReader, writer: GGUFWriter, block_count: int) -> None:
    for field in reader.fields.values():
        if field.name == Keys.General.ARCHITECTURE or field.name.startswith("GGUF."):
            continue
        if field.name.endswith(".block_count"):
            writer.add_uint32(field.name, block_count)
            continue
        val_type = field.types[0]
        sub_type = field.types[-1] if val_type == GGUFValueType.ARRAY else None
        writer.add_key_value(field.name, field.contents(), val_type, sub_type=sub_type)


def write_stage(
    reader: GGUFReader,
    out_path: Path,
    stage: int,
    layer_lo: int,
    layer_hi: int,
    include_output: bool,
) -> None:
    stage_layers = layer_hi - layer_lo
    arch = "llama"
    for field in reader.fields.values():
        if field.name == "general.architecture":
            arch = str(field.parts[field.data[0]], encoding="utf-8")
            break

    writer = GGUFWriter(out_path, arch)
    copy_metadata(reader, writer, stage_layers)

    selected: list[tuple[str, object]] = []
    for tensor in reader.tensors:
        name = tensor.name
        layer = layer_index(name)
        keep = False
        new_name = name

        if name == "token_embd.weight":
            keep = True
        elif layer is not None and layer_lo <= layer < layer_hi:
            keep = True
            new_name = rename_block(name, layer_lo)
        elif name == "output_norm.weight":
            keep = True

        if not keep:
            continue

        selected.append((new_name, tensor))

    for new_name, tensor in selected:
        writer.add_tensor_info(
            new_name,
            tensor.data.shape,
            tensor.data.dtype,
            tensor.data.nbytes,
            tensor.tensor_type,
        )

    writer.write_header_to_file()
    writer.write_kv_data_to_file()
    writer.write_ti_data_to_file()
    for _new_name, tensor in selected:
        writer.write_tensor_data(tensor.data, tensor_endianess=reader.endianess)
    writer.close()
    print(f"stage {stage}: {out_path} (layers {layer_lo}-{layer_hi - 1}, blocks={stage_layers})")


def main() -> int:
    parser = argparse.ArgumentParser(description="Split GGUF by layer stages for YouAI pipeline v3")
    parser.add_argument("input", type=Path, help="Input GGUF model")
    parser.add_argument("stages", type=int, nargs="?", default=2, help="Number of pipeline stages")
    parser.add_argument(
        "out_dir",
        type=Path,
        nargs="?",
        default=Path.home() / ".youai" / "pipeline-stages",
        help="Output directory",
    )
    args = parser.parse_args()

    if not args.input.is_file():
        print(f"input not found: {args.input}", file=sys.stderr)
        return 1

    reader = GGUFReader(str(args.input))
    n_layer = None
    for field in reader.fields.values():
        if field.name.endswith(".block_count"):
            n_layer = int(field.contents())
            break
    if n_layer is None:
        print("could not read block_count from GGUF", file=sys.stderr)
        return 1

    args.out_dir.mkdir(parents=True, exist_ok=True)
    stem = args.input.stem
    per_stage = (n_layer + args.stages - 1) // args.stages

    for stage in range(args.stages):
        layer_lo = stage * per_stage
        layer_hi = min(n_layer, (stage + 1) * per_stage)
        if layer_lo >= n_layer:
            break
        out_path = args.out_dir / f"{stem}-stage{stage:02d}-of-{args.stages:02d}.gguf"
        write_stage(
            reader,
            out_path,
            stage,
            layer_lo,
            layer_hi,
            include_output=(stage == args.stages - 1),
        )

    print(f"\nDone. {args.stages} standalone stage GGUFs in {args.out_dir}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())