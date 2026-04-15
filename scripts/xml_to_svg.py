#!/usr/bin/env python3
"""Convert Android vector XML drawables to SVG files."""

import xml.etree.ElementTree as ET
import os
import sys
import re

NS = {"android": "http://schemas.android.com/apk/res/android"}


def convert_file(xml_path, svg_path):
    tree = ET.parse(xml_path)
    root = tree.getroot()

    tag = root.tag.split("}")[-1] if "}" in root.tag else root.tag
    if tag != "vector":
        return False

    vw = root.get("{http://schemas.android.com/apk/res/android}viewportWidth", "24")
    vh = root.get("{http://schemas.android.com/apk/res/android}viewportHeight", "24")
    w = root.get("{http://schemas.android.com/apk/res/android}width", "24dp").replace(
        "dp", ""
    )
    h = root.get("{http://schemas.android.com/apk/res/android}height", "24dp").replace(
        "dp", ""
    )

    lines = []
    lines.append(
        f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {vw} {vh}" width="{w}" height="{h}">'
    )

    def process_element(el, indent=1):
        tag = el.tag.split("}")[-1] if "}" in el.tag else el.tag
        sp = "  " * indent

        if tag == "path":
            d = el.get("{http://schemas.android.com/apk/res/android}pathData", "")
            fill = el.get(
                "{http://schemas.android.com/apk/res/android}fillColor", "#000000"
            )
            stroke = el.get("{http://schemas.android.com/apk/res/android}strokeColor")
            stroke_w = el.get("{http://schemas.android.com/apk/res/android}strokeWidth")
            fill_rule = el.get("{http://schemas.android.com/apk/res/android}fillType")
            fill_opacity = el.get(
                "{http://schemas.android.com/apk/res/android}fillAlpha"
            )

            attrs = f'd="{d}"'
            if fill and fill.lower() != "none":
                attrs += f' fill="{fill}"'
            else:
                attrs += ' fill="none"'
            if fill_rule and fill_rule.lower() == "evenodd":
                attrs += ' fill-rule="evenodd"'
            if stroke:
                attrs += f' stroke="{stroke}"'
            if stroke_w:
                attrs += f' stroke-width="{stroke_w}"'
            if fill_opacity:
                attrs += f' fill-opacity="{fill_opacity}"'
            lines.append(f"{sp}<path {attrs}/>")

        elif tag == "group":
            rx = el.get("{http://schemas.android.com/apk/res/android}rotation", "0")
            px = el.get("{http://schemas.android.com/apk/res/android}pivotX", "0")
            py = el.get("{http://schemas.android.com/apk/res/android}pivotY", "0")
            tx = el.get("{http://schemas.android.com/apk/res/android}translateX", "0")
            ty = el.get("{http://schemas.android.com/apk/res/android}translateY", "0")
            sx = el.get("{http://schemas.android.com/apk/res/android}scaleX", "1")
            sy = el.get("{http://schemas.android.com/apk/res/android}scaleY", "1")

            transforms = []
            if rx != "0":
                transforms.append(f"rotate({rx} {px} {py})")
            if tx != "0" or ty != "0":
                transforms.append(f"translate({tx} {ty})")
            if sx != "1" or sy != "1":
                transforms.append(f"scale({sx} {sy})")

            if transforms:
                lines.append(f'{sp}<g transform="{" ".join(transforms)}">')
            else:
                lines.append(f"{sp}<g>")
            for child in el:
                process_element(child, indent + 1)
            lines.append(f"{sp}</g>")

        elif tag == "clip-path":
            d = el.get("{http://schemas.android.com/apk/res/android}pathData", "")
            lines.append(f'{sp}<clipPath><path d="{d}"/></clipPath>')

    for child in root:
        process_element(child)

    lines.append("</svg>")

    with open(svg_path, "w") as f:
        f.write("\n".join(lines))
    return True


src_dir = sys.argv[1]
dst_dir = sys.argv[2]

os.makedirs(dst_dir, exist_ok=True)

for fname in os.listdir(src_dir):
    if not fname.endswith(".xml"):
        continue
    xml_path = os.path.join(src_dir, fname)
    svg_name = fname.replace(".xml", ".svg")
    svg_path = os.path.join(dst_dir, svg_name)
    try:
        if convert_file(xml_path, svg_path):
            print(f"OK: {fname} -> {svg_name}")
        else:
            print(f"SKIP: {fname} (not a vector)")
    except Exception as e:
        print(f"ERR: {fname}: {e}")
