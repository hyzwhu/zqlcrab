#!/usr/bin/env python3
"""
Generates the 2x Retina background image for zqlcrab macOS DMG installer.
Window size in points: 800 x 480 (Retina: 1600 x 960 at 144 DPI)
"""

from PIL import Image, ImageDraw, ImageFont
import os

W, H = 1600, 960
img = Image.new("RGB", (W, H), (15, 23, 42))
draw = ImageDraw.Draw(img)

# Fonts
font_title = ImageFont.truetype('/System/Library/Fonts/Hiragino Sans GB.ttc', 26)
font_title_en = ImageFont.truetype('/System/Library/Fonts/Menlo.ttc', 19)
font_text_zh = ImageFont.truetype('/System/Library/Fonts/Hiragino Sans GB.ttc', 21)
font_text_en = ImageFont.truetype('/System/Library/Fonts/Hiragino Sans GB.ttc', 17)
font_badge = ImageFont.truetype('/System/Library/Fonts/Hiragino Sans GB.ttc', 18)
font_code = ImageFont.truetype('/System/Library/Fonts/Menlo.ttc', 22)

# 1. Subtle background gradient
for y in range(H):
    ratio = y / H
    r = int(14 * (1 - ratio) + 10 * ratio)
    g = int(21 * (1 - ratio) + 15 * ratio)
    b = int(37 * (1 - ratio) + 28 * ratio)
    draw.line([(0, y), (W, y)], fill=(r, g, b))

# 2. Highlighted cards behind the two main icons
card_w = 340
card_y1, card_y2 = 120, 460
left_x1, left_x2 = 380 - card_w // 2, 380 + card_w // 2
right_x1, right_x2 = 1220 - card_w // 2, 1220 + card_w // 2

for offset in [8, 6, 4, 2]:
    draw.rounded_rectangle(
        [left_x1 - offset, card_y1 - offset, left_x2 + offset, card_y2 + offset],
        radius=36 + offset,
        outline=(56, 189, 248, 15),
        width=2,
    )
    draw.rounded_rectangle(
        [right_x1 - offset, card_y1 - offset, right_x2 + offset, card_y2 + offset],
        radius=36 + offset,
        outline=(56, 189, 248, 15),
        width=2,
    )

draw.rounded_rectangle([left_x1, card_y1, left_x2, card_y2], radius=36, fill=(19, 31, 56), outline=(56, 189, 248), width=3)
draw.rounded_rectangle([right_x1, card_y1, right_x2, card_y2], radius=36, fill=(19, 31, 56), outline=(56, 189, 248), width=3)

# 3. Middle glowing arrow & Bilingual Drag instruction
arrow_y = 290
arrow_x1 = left_x2 + 40
arrow_x2 = right_x1 - 40

draw.line([(arrow_x1, arrow_y), (arrow_x2, arrow_y)], fill=(56, 189, 248), width=6)
arrow_pts = [
    (arrow_x2 + 20, arrow_y),
    (arrow_x2 - 10, arrow_y - 18),
    (arrow_x2 - 10, arrow_y + 18),
]
draw.polygon(arrow_pts, fill=(56, 189, 248))

inst_zh = "拖拽到 Applications 完成安装"
inst_en = "Drag to Applications to Install"
zh_bbox = draw.textbbox((0, 0), inst_zh, font=font_title)
en_bbox = draw.textbbox((0, 0), inst_en, font=font_title_en)
zh_w = zh_bbox[2] - zh_bbox[0]
en_w = en_bbox[2] - en_bbox[0]
draw.text((800 - zh_w // 2, arrow_y - 80), inst_zh, font=font_title, fill=(241, 245, 249))
draw.text((800 - en_w // 2, arrow_y - 42), inst_en, font=font_title_en, fill=(148, 163, 184))

# 4. Bottom card for macOS Gatekeeper tip & Double-click copy script slot
box_w = 1440
box_h = 325
box_x1 = (W - box_w) // 2
box_x2 = box_x1 + box_w
box_y1 = 545
box_y2 = box_y1 + box_h

draw.rounded_rectangle(
    [box_x1 - 4, box_y1 - 4, box_x2 + 4, box_y2 + 4],
    radius=22,
    outline=(245, 158, 11, 25),
    width=2,
)
draw.rounded_rectangle(
    [box_x1, box_y1, box_x2, box_y2],
    radius=20,
    fill=(15, 23, 42),
    outline=(245, 158, 11),
    width=2,
)

# Badge
badge_x1 = box_x1 + 30
badge_y1 = box_y1 + 22
badge_x2 = badge_x1 + 130
badge_y2 = badge_y1 + 44
draw.rounded_rectangle([badge_x1, badge_y1, badge_x2, badge_y2], radius=10, fill=(69, 26, 3), outline=(245, 158, 11), width=2)
badge_text = "提示 / TIP"
badge_bb = draw.textbbox((0, 0), badge_text, font=font_badge)
badge_w = badge_bb[2] - badge_bb[0]
draw.text((badge_x1 + (130 - badge_w) // 2, badge_y1 + 10), badge_text, font=font_badge, fill=(245, 158, 11))

# Bilingual Tip Text
tip_zh = '首次打开若提示 “已损坏”，可双击右侧文件复制命令，或在「终端」执行以下命令解除隔离:'
tip_en = 'If prompted "Damaged", open file on right to copy, or run command below in Terminal:'
draw.text((badge_x2 + 20, badge_y1 + 1), tip_zh, font=font_text_zh, fill=(241, 245, 249))
draw.text((badge_x2 + 20, badge_y1 + 28), tip_en, font=font_text_en, fill=(148, 163, 184))

# Code command box (left)
code_box_x1 = box_x1 + 30
code_box_x2 = box_x1 + 1080
code_box_y1 = box_y1 + 82
code_box_y2 = box_y2 - 25

draw.rounded_rectangle(
    [code_box_x1, code_box_y1, code_box_x2, code_box_y2],
    radius=14,
    fill=(9, 13, 22),
    outline=(30, 41, 59),
    width=2,
)

cmd_prompt = "$"
cmd_text = "sudo xattr -rd com.apple.quarantine /Applications/zqlcrab.app"
draw.text((code_box_x1 + 24, code_box_y1 + 55), cmd_prompt, font=font_code, fill=(100, 116, 139))
draw.text((code_box_x1 + 52, code_box_y1 + 55), cmd_text, font=font_code, fill=(74, 222, 128))

# Copy Script zone (right)
script_zone_x1 = code_box_x2 + 25
script_zone_x2 = box_x2 - 30
script_zone_y1 = code_box_y1 - 8
script_zone_y2 = code_box_y2 + 8

for offset in [4, 2]:
    draw.rounded_rectangle(
        [script_zone_x1 - offset, script_zone_y1 - offset, script_zone_x2 + offset, script_zone_y2 + offset],
        radius=14 + offset,
        outline=(56, 189, 248, 20),
        width=2,
    )

draw.rounded_rectangle(
    [script_zone_x1, script_zone_y1, script_zone_x2, script_zone_y2],
    radius=14,
    fill=(19, 31, 56),
    outline=(56, 189, 248),
    width=2,
)

# Output file path
output_path = os.path.join(os.path.dirname(__file__), '..', 'assets', 'dmg-background.png')
img.save(output_path, dpi=(144, 144))
print(f"Saved DMG background to {output_path} (144 DPI)")
