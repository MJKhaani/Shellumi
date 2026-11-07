#!/usr/bin/env python3
"""
generate_icons.py — Comprehensive icon suite generator for Shellumi.

Generates HIG-compliant application icons and visual assets across all supported platforms:
- macOS (Apple Human Interface Guidelines): 1024x1024 canvas, 824x824 continuous squircle body,
  authentic ambient + key drop shadow, 10-resolution AppIcon.icns, AppIcon.png.
- Windows (Fluent Design HIG): Multi-resolution shellumi.ico (16, 24, 32, 48, 64, 128, 256),
  square tile assets (Square44x44, Square150x150, Square310x310, StoreLogo).
- Linux (FreeDesktop Icon Theme Specification HIG): hicolor raster tree (16..512) and scalable SVG.
- iOS & Android: Standard App Store & Google Play Store assets, adaptive icons, and mipmaps.
- Documentation & Web: Master resources/logo.png and resources/logo.svg.
"""

import os
import shutil
import struct
import subprocess
import zlib

WORKSPACE_ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
RESOURCES_DIR = os.path.join(WORKSPACE_ROOT, "resources")
SRC_LOGO_PNG = os.path.join(RESOURCES_DIR, "logo.png")

def load_png_rgba(path):
    with open(path, "rb") as f:
        sig = f.read(8)
        assert sig == b"\x89PNG\r\n\x1a\n", f"Invalid PNG signature: {path}"
        idat = b""
        w, h = 0, 0
        while True:
            chunk = f.read(8)
            if not chunk:
                break
            length, ctype = struct.unpack(">I4s", chunk)
            data = f.read(length)
            f.read(4)  # CRC
            if ctype == b"IHDR":
                w, h = struct.unpack(">II", data[:8])
            elif ctype == b"IDAT":
                idat += data
            elif ctype == b"IEND":
                break

    raw = zlib.decompress(idat)
    stride = 1 + w * 4
    prev = bytearray(w * 4)
    curr = bytearray(w * 4)
    pixels = bytearray(w * h * 4)
    for y in range(h):
        line = raw[y * stride : (y + 1) * stride]
        ft = line[0]
        data = line[1:]
        if ft == 0:
            curr[:] = data
        elif ft == 1:
            for i in range(w * 4):
                curr[i] = (data[i] + (curr[i - 4] if i >= 4 else 0)) & 0xFF
        elif ft == 2:
            for i in range(w * 4):
                curr[i] = (data[i] + prev[i]) & 0xFF
        elif ft == 3:
            for i in range(w * 4):
                curr[i] = (data[i] + (((curr[i - 4] if i >= 4 else 0) + prev[i]) >> 1)) & 0xFF
        elif ft == 4:
            for i in range(w * 4):
                a = curr[i - 4] if i >= 4 else 0
                b = prev[i]
                c = prev[i - 4] if i >= 4 else 0
                p = a + b - c
                pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
                pr = a if (pa <= pb and pa <= pc) else (b if pb <= pc else c)
                curr[i] = (data[i] + pr) & 0xFF
        pixels[y * w * 4 : (y + 1) * w * 4] = curr
        prev[:] = curr
    return w, h, pixels


def save_png_rgba(path, w, h, pixels):
    os.makedirs(os.path.dirname(os.path.abspath(path)), exist_ok=True)
    raw = bytearray()
    stride = w * 4
    for y in range(h):
        raw.append(0)  # filter None
        raw.extend(pixels[y * stride : (y + 1) * stride])
    compressed = zlib.compress(bytes(raw), 9)
    ihdr = struct.pack(">IIBBBBB", w, h, 8, 6, 0, 0, 0)

    def chunk(t, data):
        return (
            struct.pack(">I", len(data))
            + t
            + data
            + struct.pack(">I", zlib.crc32(t + data) & 0xFFFFFFFF)
        )

    with open(path, "wb") as f:
        f.write(b"\x89PNG\r\n\x1a\n")
        f.write(chunk(b"IHDR", ihdr))
        f.write(chunk(b"IDAT", compressed))
        f.write(chunk(b"IEND", b""))


def sips_resample(src, dst, w, h):
    os.makedirs(os.path.dirname(os.path.abspath(dst)), exist_ok=True)
    subprocess.run(
        ["sips", "-z", str(h), str(w), src, "--out", dst],
        check=True,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )


def box_blur_2d(src, w, h, r):
    temp = bytearray(w * h)
    dst = bytearray(w * h)
    scale = 1.0 / (2 * r + 1)
    # Horizontal pass
    for y in range(h):
        row = y * w
        val = 0
        for x in range(-r, r + 1):
            val += src[row + min(max(x, 0), w - 1)]
        temp[row] = int(val * scale)
        for x in range(1, w):
            val += src[row + min(x + r, w - 1)] - src[row + max(x - r - 1, 0)]
            temp[row + x] = int(val * scale)
    # Vertical pass
    for x in range(w):
        val = 0
        for y in range(-r, r + 1):
            val += temp[min(max(y, 0), h - 1) * w + x]
        dst[x] = int(val * scale)
        for y in range(1, h):
            val += temp[min(y + r, h - 1) * w + x] - temp[max(y - r - 1, 0) * w + x]
            dst[y * w + x] = int(val * scale)
    return dst


def gaussian_blur_approx(src, w, h, r):
    b1 = box_blur_2d(src, w, h, r)
    b2 = box_blur_2d(b1, w, h, r)
    return box_blur_2d(b2, w, h, r)


def create_macos_hig_master(squircle_raw_png, output_png):
    """
    Creates standard 1024x1024 macOS HIG icon with 824x824 body,
    100px margins and realistic ambient + key drop shadow.
    """
    temp_824 = "/tmp/shellumi_squircle_824.png"
    sips_resample(squircle_raw_png, temp_824, 824, 824)
    w_824, h_824, pix_824 = load_png_rgba(temp_824)

    W, H = 1024, 1024
    ox, oy = 100, 100

    # Extract squircle alpha mask
    mask = bytearray(W * H)
    for y in range(h_824):
        for x in range(w_824):
            mask[(oy + y) * W + (ox + x)] = pix_824[(y * w_824 + x) * 4 + 3]

    ambient = gaussian_blur_approx(mask, W, H, 8)

    # Key shadow: shifted down by dy=+14
    dy = 14
    mask_shifted = bytearray(W * H)
    for y in range(H - dy):
        mask_shifted[(y + dy) * W : (y + dy + 1) * W] = mask[y * W : (y + 1) * W]
    key = gaussian_blur_approx(mask_shifted, W, H, 16)

    # Combine shadows and composite body
    final = bytearray(W * H * 4)
    for i in range(W * H):
        a_amb = ambient[i] * 0.22 / 255.0
        a_key = key[i] * 0.38 / 255.0
        a_shad = 1.0 - (1.0 - a_amb) * (1.0 - a_key)
        final[i * 4 + 3] = int(min(max(a_shad * 255.0, 0), 255))

    # Composite 824x824 body over shadow
    for y in range(h_824):
        for x in range(w_824):
            src_idx = (y * w_824 + x) * 4
            dst_idx = ((oy + y) * W + (ox + x)) * 4
            sa = pix_824[src_idx + 3] / 255.0
            da = final[dst_idx + 3] / 255.0
            sr, sg, sb = pix_824[src_idx], pix_824[src_idx + 1], pix_824[src_idx + 2]
            dr, dg, db = final[dst_idx], final[dst_idx + 1], final[dst_idx + 2]

            out_a = sa + da * (1.0 - sa)
            if out_a > 0.001:
                out_r = (sr * sa + dr * da * (1.0 - sa)) / out_a
                out_g = (sg * sa + dg * da * (1.0 - sa)) / out_a
                out_b = (sb * sa + db * da * (1.0 - sa)) / out_a
            else:
                out_r, out_g, out_b = 0, 0, 0

            final[dst_idx] = int(round(out_r))
            final[dst_idx + 1] = int(round(out_g))
            final[dst_idx + 2] = int(round(out_b))
            final[dst_idx + 3] = int(round(out_a * 255.0))

    save_png_rgba(output_png, W, H, final)
    print(f"Generated macOS HIG master: {output_png}")


def create_macos_icns(master_png, output_icns):
    iconset_dir = "/tmp/Shellumi.iconset"
    if os.path.exists(iconset_dir):
        shutil.rmtree(iconset_dir)
    os.makedirs(iconset_dir, exist_ok=True)

    sizes = [
        ("icon_16x16.png", 16),
        ("icon_16x16@2x.png", 32),
        ("icon_32x32.png", 32),
        ("icon_32x32@2x.png", 64),
        ("icon_128x128.png", 128),
        ("icon_128x128@2x.png", 256),
        ("icon_256x256.png", 256),
        ("icon_256x256@2x.png", 512),
        ("icon_512x512.png", 512),
        ("icon_512x512@2x.png", 1024),
    ]

    for name, s in sizes:
        dst = os.path.join(iconset_dir, name)
        if s == 1024:
            shutil.copyfile(master_png, dst)
        else:
            sips_resample(master_png, dst, s, s)

    os.makedirs(os.path.dirname(os.path.abspath(output_icns)), exist_ok=True)
    subprocess.run(
        ["iconutil", "-c", "icns", iconset_dir, "-o", output_icns],
        check=True,
    )
    print(f"Generated macOS ICNS: {output_icns}")


def create_windows_ico(master_png, output_ico):
    sizes = [16, 24, 32, 48, 64, 128, 256]
    png_blobs = []
    tmp_dir = "/tmp/win_ico_parts"
    os.makedirs(tmp_dir, exist_ok=True)

    for s in sizes:
        png_path = os.path.join(tmp_dir, f"icon_{s}.png")
        sips_resample(master_png, png_path, s, s)
        with open(png_path, "rb") as f:
            png_blobs.append((s, f.read()))

    count = len(png_blobs)
    header = struct.pack("<HHH", 0, 1, count)
    offset = 6 + 16 * count

    entries = []
    for s, blob in png_blobs:
        w = 0 if s == 256 else s
        h = 0 if s == 256 else s
        entry = struct.pack("<BBBBHHII", w, h, 0, 0, 1, 32, len(blob), offset)
        entries.append(entry)
        offset += len(blob)

    os.makedirs(os.path.dirname(os.path.abspath(output_ico)), exist_ok=True)
    with open(output_ico, "wb") as f:
        f.write(header)
        for entry in entries:
            f.write(entry)
        for s, blob in png_blobs:
            f.write(blob)
    print(f"Generated Windows Multi-Res ICO: {output_ico}")


def create_windows_tiles(master_png, output_dir):
    os.makedirs(output_dir, exist_ok=True)
    tiles = [
        ("Square44x44Logo.png", 44),
        ("Square150x150Logo.png", 150),
        ("Square310x310Logo.png", 310),
        ("StoreLogo.png", 50),
    ]
    for filename, s in tiles:
        dst = os.path.join(output_dir, filename)
        sips_resample(master_png, dst, s, s)
    print(f"Generated Windows Tiles in: {output_dir}")


def create_linux_hicolor_tree(master_png, hicolor_base_dir):
    sizes = [16, 24, 32, 48, 64, 128, 256, 512]
    for s in sizes:
        dim = f"{s}x{s}"
        app_dir = os.path.join(hicolor_base_dir, dim, "apps")
        os.makedirs(app_dir, exist_ok=True)
        dst = os.path.join(app_dir, "shellumi.png")
        sips_resample(master_png, dst, s, s)
    print(f"Generated Linux FreeDesktop Hicolor Icon Tree: {hicolor_base_dir}")


def create_ios_assets(master_png, output_dir):
    os.makedirs(output_dir, exist_ok=True)
    # iOS App Store icon is 1024x1024
    sips_resample(master_png, os.path.join(output_dir, "AppIcon.png"), 1024, 1024)
    # Standard iOS AppIcon sizes
    ios_sizes = [
        ("AppIcon-20@2x.png", 40),
        ("AppIcon-20@3x.png", 60),
        ("AppIcon-29@2x.png", 58),
        ("AppIcon-29@3x.png", 87),
        ("AppIcon-40@2x.png", 80),
        ("AppIcon-40@3x.png", 120),
        ("AppIcon-60@2x.png", 120),
        ("AppIcon-60@3x.png", 180),
        ("AppIcon-1024.png", 1024),
    ]
    for filename, s in ios_sizes:
        dst = os.path.join(output_dir, filename)
        sips_resample(master_png, dst, s, s)
    print(f"Generated iOS Assets in: {output_dir}")


def create_android_assets(master_png, res_base_dir):
    mipmaps = [
        ("mipmap-mdpi", 48),
        ("mipmap-hdpi", 72),
        ("mipmap-xhdpi", 96),
        ("mipmap-xxhdpi", 144),
        ("mipmap-xxxhdpi", 192),
    ]
    for dirname, s in mipmaps:
        folder = os.path.join(res_base_dir, dirname)
        os.makedirs(folder, exist_ok=True)
        sips_resample(master_png, os.path.join(folder, "ic_launcher.png"), s, s)
        sips_resample(master_png, os.path.join(folder, "ic_launcher_round.png"), s, s)

    # Google Play Store 512x512
    os.makedirs(res_base_dir, exist_ok=True)
    sips_resample(master_png, os.path.join(res_base_dir, "playstore-icon.png"), 512, 512)

    # Adaptive Icon Foreground & Background (432x432)
    drawable_dir = os.path.join(res_base_dir, "drawable")
    os.makedirs(drawable_dir, exist_ok=True)
    sips_resample(master_png, os.path.join(drawable_dir, "ic_launcher_foreground.png"), 432, 432)
    sips_resample(master_png, os.path.join(drawable_dir, "ic_launcher_background.png"), 432, 432)
    print(f"Generated Android Assets in: {res_base_dir}")


def create_svg_vector_icon(output_svg):
    svg_content = """<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 512 512" width="512" height="512">
  <defs>
    <!-- Background Gradients -->
    <linearGradient id="macOuterBevel" x1="0%" y1="0%" x2="0%" y2="100%">
      <stop offset="0%" stop-color="#cbd5e1" />
      <stop offset="15%" stop-color="#94a3b8" />
      <stop offset="85%" stop-color="#475569" />
      <stop offset="100%" stop-color="#1e293b" />
    </linearGradient>
    <linearGradient id="bgGradient" x1="0%" y1="0%" x2="0%" y2="100%">
      <stop offset="0%" stop-color="#1e2530" />
      <stop offset="50%" stop-color="#121820" />
      <stop offset="100%" stop-color="#0a0d12" />
    </linearGradient>
    <radialGradient id="centerGlow" cx="50%" cy="50%" r="50%">
      <stop offset="0%" stop-color="#06b6d4" stop-opacity="0.18" />
      <stop offset="100%" stop-color="#06b6d4" stop-opacity="0" />
    </radialGradient>
    <!-- Server Blade Gradient -->
    <linearGradient id="bladeGrad" x1="0%" y1="0%" x2="0%" y2="100%">
      <stop offset="0%" stop-color="#2a3441" />
      <stop offset="100%" stop-color="#151b24" />
    </linearGradient>
    <linearGradient id="bladeBevel" x1="0%" y1="0%" x2="0%" y2="100%">
      <stop offset="0%" stop-color="#475569" stop-opacity="0.8" />
      <stop offset="100%" stop-color="#0f172a" stop-opacity="0.8" />
    </linearGradient>
    <!-- Center Terminal Node Neon Border -->
    <linearGradient id="neonRim" x1="0%" y1="0%" x2="100%" y2="100%">
      <stop offset="0%" stop-color="#38bdf8" />
      <stop offset="40%" stop-color="#06b6d4" />
      <stop offset="70%" stop-color="#a855f7" />
      <stop offset="100%" stop-color="#ec4899" />
    </linearGradient>
    <!-- LED Glow Filter -->
    <filter id="greenGlow" x="-30%" y="-30%" width="160%" height="160%">
      <feGaussianBlur stdDeviation="3" result="blur" />
      <feMerge>
        <feMergeNode in="blur" />
        <feMergeNode in="SourceGraphic" />
      </feMerge>
    </filter>
    <!-- Cyan Glow Filter -->
    <filter id="cyanGlow" x="-30%" y="-30%" width="160%" height="160%">
      <feGaussianBlur stdDeviation="4" result="blur" />
      <feMerge>
        <feMergeNode in="blur" />
        <feMergeNode in="SourceGraphic" />
      </feMerge>
    </filter>
    <filter id="softShadow" x="-20%" y="-20%" width="140%" height="140%">
      <feDropShadow dx="0" dy="8" stdDeviation="12" flood-color="#000000" flood-opacity="0.5" />
    </filter>
  </defs>

  <!-- Outer Squircle with Metallic Bevel -->
  <rect x="32" y="32" width="448" height="448" rx="100" fill="url(#macOuterBevel)" />
  <rect x="36" y="36" width="440" height="440" rx="96" fill="#090d13" />
  <rect x="40" y="40" width="432" height="432" rx="92" fill="url(#bgGradient)" />
  <circle cx="256" cy="256" r="216" fill="url(#centerGlow)" />

  <!-- Connecting Glowing Cyan Circuit Lines -->
  <line x1="256" y1="200" x2="256" y2="148" stroke="#00e5ff" stroke-width="5" filter="url(#cyanGlow)" />
  <line x1="256" y1="312" x2="256" y2="364" stroke="#00e5ff" stroke-width="5" filter="url(#cyanGlow)" />
  <path d="M 200 256 C 180 256, 176 256, 154 256" stroke="#00e5ff" stroke-width="5" fill="none" filter="url(#cyanGlow)" />
  <path d="M 312 256 C 332 256, 336 256, 358 256" stroke="#00e5ff" stroke-width="5" fill="none" filter="url(#cyanGlow)" />

  <!-- Connection Ports -->
  <circle cx="256" cy="154" r="6" fill="#ffffff" stroke="#00e5ff" stroke-width="2" filter="url(#cyanGlow)" />
  <circle cx="256" cy="358" r="6" fill="#ffffff" stroke="#00e5ff" stroke-width="2" filter="url(#cyanGlow)" />
  <circle cx="160" cy="256" r="6" fill="#ffffff" stroke="#00e5ff" stroke-width="2" filter="url(#cyanGlow)" />
  <circle cx="352" cy="256" r="6" fill="#ffffff" stroke="#00e5ff" stroke-width="2" filter="url(#cyanGlow)" />

  <!-- Top Server Blade -->
  <g filter="url(#softShadow)">
    <rect x="196" y="80" width="120" height="66" rx="20" fill="url(#bladeBevel)" />
    <rect x="198" y="82" width="116" height="62" rx="18" fill="url(#bladeGrad)" />
    <rect x="214" y="96" width="48" height="8" rx="4" fill="#0f172a" stroke="#334155" stroke-width="1" />
    <rect x="214" y="116" width="48" height="8" rx="4" fill="#0f172a" stroke="#334155" stroke-width="1" />
    <circle cx="288" cy="100" r="5" fill="#22c55e" filter="url(#greenGlow)" />
    <circle cx="288" cy="120" r="5" fill="#22c55e" filter="url(#greenGlow)" />
  </g>

  <!-- Bottom Server Blade -->
  <g filter="url(#softShadow)">
    <rect x="196" y="366" width="120" height="66" rx="20" fill="url(#bladeBevel)" />
    <rect x="198" y="368" width="116" height="62" rx="18" fill="url(#bladeGrad)" />
    <rect x="214" y="382" width="48" height="8" rx="4" fill="#0f172a" stroke="#334155" stroke-width="1" />
    <rect x="214" y="402" width="48" height="8" rx="4" fill="#0f172a" stroke="#334155" stroke-width="1" />
    <circle cx="288" cy="386" r="5" fill="#22c55e" filter="url(#greenGlow)" />
    <circle cx="288" cy="406" r="5" fill="#22c55e" filter="url(#greenGlow)" />
  </g>

  <!-- Left Server Blade -->
  <g filter="url(#softShadow)">
    <rect x="58" y="223" width="102" height="66" rx="20" fill="url(#bladeBevel)" />
    <rect x="60" y="225" width="98" height="62" rx="18" fill="url(#bladeGrad)" />
    <rect x="74" y="239" width="42" height="8" rx="4" fill="#0f172a" stroke="#334155" stroke-width="1" />
    <rect x="74" y="259" width="42" height="8" rx="4" fill="#0f172a" stroke="#334155" stroke-width="1" />
    <circle cx="138" cy="243" r="5" fill="#22c55e" filter="url(#greenGlow)" />
    <circle cx="138" cy="263" r="5" fill="#22c55e" filter="url(#greenGlow)" />
  </g>

  <!-- Right Server Blade -->
  <g filter="url(#softShadow)">
    <rect x="352" y="223" width="102" height="66" rx="20" fill="url(#bladeBevel)" />
    <rect x="354" y="225" width="98" height="62" rx="18" fill="url(#bladeGrad)" />
    <rect x="368" y="239" width="42" height="8" rx="4" fill="#0f172a" stroke="#334155" stroke-width="1" />
    <rect x="368" y="259" width="42" height="8" rx="4" fill="#0f172a" stroke="#334155" stroke-width="1" />
    <circle cx="432" cy="243" r="5" fill="#22c55e" filter="url(#greenGlow)" />
    <circle cx="432" cy="263" r="5" fill="#22c55e" filter="url(#greenGlow)" />
  </g>

  <!-- Central Terminal Node -->
  <g filter="url(#softShadow)">
    <rect x="198" y="198" width="116" height="116" rx="28" fill="url(#neonRim)" filter="url(#cyanGlow)" />
    <rect x="200" y="200" width="112" height="112" rx="26" fill="url(#neonRim)" />
    <rect x="205" y="205" width="102" height="102" rx="22" fill="#131922" />
    <!-- Prompt >_ -->
    <path d="M 228 238 L 252 256 L 228 274" stroke="#ffffff" stroke-width="8" stroke-linecap="round" stroke-linejoin="round" fill="none" />
    <line x1="260" y1="274" x2="282" y2="274" stroke="#ffffff" stroke-width="8" stroke-linecap="round" />
  </g>
</svg>
"""
    os.makedirs(os.path.dirname(os.path.abspath(output_svg)), exist_ok=True)
    with open(output_svg, "w", encoding="utf-8") as f:
        f.write(svg_content)
    print(f"Generated Vector SVG: {output_svg}")


def main():
    print("=== Shellumi HIG Icon Suite Generator ===")
    master_png = "/tmp/shellumi_macos_hig_master.png"

    # 1. Generate macOS HIG Master (1024x1024 with 824x824 body and authentic drop shadow)
    create_macos_hig_master(SRC_LOGO_PNG, master_png)

    # 2. Update resources/logo.png & resources/AppIcon.png
    dest_logo_png = os.path.join(RESOURCES_DIR, "logo.png")
    dest_app_icon_png = os.path.join(RESOURCES_DIR, "AppIcon.png")
    shutil.copyfile(master_png, dest_logo_png)
    shutil.copyfile(master_png, dest_app_icon_png)
    print(f"Updated {dest_logo_png} and {dest_app_icon_png}")

    # 3. Generate macOS AppIcon.icns
    dest_app_icon_icns = os.path.join(RESOURCES_DIR, "AppIcon.icns")
    create_macos_icns(master_png, dest_app_icon_icns)

    # 4. Generate Windows Multi-Res ICO & Tiles
    dest_win_ico = os.path.join(RESOURCES_DIR, "shellumi.ico")
    create_windows_ico(master_png, dest_win_ico)
    create_windows_tiles(master_png, os.path.join(RESOURCES_DIR, "windows"))

    # 5. Generate Linux FreeDesktop Hicolor Icon Theme & SVG
    hicolor_dir = os.path.join(RESOURCES_DIR, "icons", "hicolor")
    create_linux_hicolor_tree(master_png, hicolor_dir)
    svg_scalable = os.path.join(hicolor_dir, "scalable", "apps", "shellumi.svg")
    create_svg_vector_icon(svg_scalable)
    dest_logo_svg = os.path.join(RESOURCES_DIR, "logo.svg")
    create_svg_vector_icon(dest_logo_svg)

    # 6. Generate Mobile Assets (iOS & Android)
    create_ios_assets(master_png, os.path.join(RESOURCES_DIR, "mobile", "ios"))
    create_ios_assets(master_png, os.path.join(WORKSPACE_ROOT, "packaging", "mobile", "ios"))
    create_android_assets(master_png, os.path.join(RESOURCES_DIR, "mobile", "android"))
    create_android_assets(
        master_png,
        os.path.join(WORKSPACE_ROOT, "packaging", "mobile", "android", "src", "main", "res"),
    )

    # 7. Update existing dist/ app bundles
    dist_dir = os.path.join(WORKSPACE_ROOT, "dist")
    if os.path.exists(dist_dir):
        for root, dirs, files in os.walk(dist_dir):
            if os.path.basename(root) == "Resources" and os.path.basename(os.path.dirname(root)) == "Contents":
                target_png = os.path.join(root, "AppIcon.png")
                target_icns = os.path.join(root, "AppIcon.icns")
                shutil.copyfile(master_png, target_png)
                shutil.copyfile(dest_app_icon_icns, target_icns)
                print(f"Updated bundle resources: {root}")

    print("=== Icon Generation Complete ===")


if __name__ == "__main__":
    main()
