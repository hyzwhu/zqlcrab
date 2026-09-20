#!/usr/bin/env python3
"""
Generates the 2x Retina background image for zqlcrab macOS DMG installer.
Window size in points: 800 x 420 (Retina: 1600 x 840 at 144 DPI)
"""

from PIL import Image, ImageDraw, ImageFont
import os

W, H = 1600, 840
img = Image.new("RGB", (W, H), (15, 23, 42))
draw = ImageDraw.Draw(img)

# Fonts
font_title = ImageFont.truetype('/System/Library/Fonts/Hiragino Sans GB.ttc', 28)
font_text = ImageFont.truetype('/System/Library/Fonts/Hiragino Sans GB.ttc', 24)
font_badge = ImageFont.truetype('/System/Library/Fonts/Hiragino Sans GB.ttc', 20)
font_code = ImageFont.truetype('/System/Library/Fonts/Menlo.ttc', 26)
font_code_tag = ImageFont.truetype('/System/Library/Fonts/Menlo.ttc', 20)

# 1. Subtle background gradient
for y in range(H):
    ratio = y / H
    r = int(14 * (1 - ratio) + 10 * ratio)
    g = int(21 * (1 - ratio) + 15 * ratio)
    b = int(37 * (1 - ratio) + 28 * ratio)
    draw.line([(0, y), (W, y)], fill=(r, g, b))

# 2. Highlighted cards behind the two icons
card_w = 340
card_y1, card_y2 = 210, 565
left_x1, left_x2 = 400 - card_w // 2, 400 + card_w // 2
right_x1, right_x2 = 1200 - card_w // 2, 1200 + card_w // 2

# Subtle glow outer borders
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

# Inner card fill and primary border
draw.rounded_rectangle(
    [left_x1, card_y1, left_x2, card_y2],
    radius=36,
    fill=(22, 32, 50),
    outline=(56, 189, 248),
    width=3,
)
draw.rounded_rectangle(
    [right_x1, card_y1, right_x2, card_y2],
    radius=36,
    fill=(22, 32, 50),
    outline=(56, 189, 248),
    width=3,
)

# 3. Arrow pointing from left to right between cards
arrow_cy = 380
arrow_len = 160
arrow_x1 = 800 - arrow_len // 2
arrow_x2 = 800 + arrow_len // 2

draw.line([(arrow_x1, arrow_cy), (arrow_x2, arrow_cy)], fill=(56, 189, 248), width=6)
arrow_pts = [
    (arrow_x2 + 20, arrow_cy),
    (arrow_x2 - 10, arrow_cy - 18),
    (arrow_x2 - 10, arrow_cy + 18),
]
draw.polygon(arrow_pts, fill=(56, 189, 248))

# Label above arrow
inst_text = "拖拽到 Applications 完成安装"
inst_bbox = draw.textbbox((0, 0), inst_text, font=font_title)
inst_w = inst_bbox[2] - inst_bbox[0]
draw.text((800 - inst_w // 2, arrow_cy - 65), inst_text, font=font_title, fill=(226, 232, 240))

# 4. Bottom highlighted card for macOS Gatekeeper tip
box_w = 1260
box_h = 135
box_x1 = (W - box_w) // 2
box_x2 = box_x1 + box_w
box_y1 = H - box_h - 45
box_y2 = box_y1 + box_h

draw.rounded_rectangle(
    [box_x1, box_y1, box_x2, box_y2],
    radius=20,
    fill=(15, 23, 42),
    outline=(245, 158, 11),
    width=2,
)

# Badge: [提示]
badge_w = 88
badge_h = 36
badge_x = box_x1 + 25
badge_y = box_y1 + 18

draw.rounded_rectangle(
    [badge_x, badge_y, badge_x + badge_w, badge_y + badge_h],
    radius=8,
    fill=(69, 26, 3),
    outline=(245, 158, 11),
    width=2,
)
draw.text((badge_x + 14, badge_y + 6), "提示", font=font_badge, fill=(251, 191, 36))

# Explanation text next to badge
tip_text = "首次打开若提示“已损坏”，请打开「终端」(Terminal) 执行以下命令解除隔离："
draw.text((badge_x + badge_w + 16, badge_y + 5), tip_text, font=font_text, fill=(241, 245, 249))

# Code command box inside bottom card
code_box_x1 = box_x1 + 25
code_box_x2 = box_x2 - 25
code_box_y1 = box_y1 + 68
code_box_y2 = box_y2 - 20

draw.rounded_rectangle(
    [code_box_x1, code_box_y1, code_box_x2, code_box_y2],
    radius=12,
    fill=(9, 13, 22),
    outline=(30, 41, 59),
    width=2,
)

cmd_prompt = "$"
cmd_text = "sudo xattr -rd com.apple.quarantine /Applications/zqlcrab.app"
draw.text((code_box_x1 + 20, code_box_y1 + 17), cmd_prompt, font=font_code, fill=(100, 116, 139))
draw.text((code_box_x1 + 48, code_box_y1 + 17), cmd_text, font=font_code, fill=(74, 222, 128))

# Output file path
output_path = os.path.join(os.path.dirname(__file__), '..', 'assets', 'dmg-background.png')
img.save(output_path, dpi=(144, 144))
print(f"Saved DMG background to {output_path} (144 DPI)")
