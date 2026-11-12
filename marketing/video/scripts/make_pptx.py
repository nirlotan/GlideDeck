"""Writes assets/launch-plan.pptx: the PowerPoint deck the video imports into GlideDeck.

A real .pptx, built with python-pptx from text boxes, shapes and solid slide
backgrounds only (the things GlideDeck's importer brings across).
    python scripts/make_pptx.py
"""
from pathlib import Path
from pptx import Presentation
from pptx.dml.color import RGBColor
from pptx.enum.shapes import MSO_SHAPE
from pptx.enum.text import PP_ALIGN
from pptx.util import Emu, Pt

W, H = 12192000, 6858000  # 16:9, in EMU
INK, BONE, GRAPHITE = "14110A", "F1EFE9", "1D2125"
YELLOW, AQUA, CORAL, VIOLET, NIGHT = "FFD23F", "35D0BA", "FF6B4A", "9DA7FF", "0F1113"

prs = Presentation()
prs.slide_width, prs.slide_height = Emu(W), Emu(H)


def rgb(h):
    return RGBColor.from_string(h)


def slide(bg):
    s = prs.slides.add_slide(prs.slide_layouts[6])
    s.background.fill.solid()
    s.background.fill.fore_color.rgb = rgb(bg)
    return s


def text(s, body, x, y, w, h, size, color, bold=True, font="Segoe UI Black", align=PP_ALIGN.LEFT):
    box = s.shapes.add_textbox(Emu(int(x * W)), Emu(int(y * H)), Emu(int(w * W)), Emu(int(h * H)))
    tf = box.text_frame
    tf.word_wrap = True
    for i, line in enumerate(body.split("\n")):
        p = tf.paragraphs[0] if i == 0 else tf.add_paragraph()
        p.alignment = align
        r = p.add_run()
        r.text = line
        r.font.size, r.font.bold, r.font.name = Pt(size), bold, font
        r.font.color.rgb = rgb(color)
    return box


def shape(s, kind, x, y, w, h, fill):
    sh = s.shapes.add_shape(kind, Emu(int(x * W)), Emu(int(y * H)), Emu(int(w * W)), Emu(int(h * H)))
    sh.fill.solid()
    sh.fill.fore_color.rgb = rgb(fill)
    sh.line.fill.background()
    return sh


def small(s, body, x, y, color, w=0.6):
    return text(s, body, x, y, w, 0.1, 20, color, bold=False, font="Segoe UI")


# 1. Title
s = slide(NIGHT)
shape(s, MSO_SHAPE.OVAL, 0.70, 0.16, 0.22, 0.39, YELLOW)
shape(s, MSO_SHAPE.OVAL, 0.765, 0.275, 0.09, 0.16, NIGHT)
text(s, "Aurora", 0.07, 0.30, 0.6, 0.3, 96, BONE)
small(s, "Launch plan · Spring", 0.075, 0.62, "8F969C")

# 2. The problem
s = slide(BONE)
text(s, "The problem", 0.07, 0.10, 0.8, 0.2, 54, INK)
for i, word in enumerate(["Slow", "Scattered", "Expensive"]):
    shape(s, MSO_SHAPE.ROUNDED_RECTANGLE, 0.07 + i * 0.295, 0.40, 0.27, 0.42, GRAPHITE)
    text(s, word, 0.09 + i * 0.295, 0.66, 0.24, 0.12, 30, BONE)
    shape(s, MSO_SHAPE.OVAL, 0.095 + i * 0.295, 0.46, 0.05, 0.089, [CORAL, YELLOW, AQUA][i])

# 3. The big number
s = slide(YELLOW)
text(s, "3×", 0.07, 0.12, 0.6, 0.5, 190, INK)
text(s, "faster from idea to launch", 0.075, 0.70, 0.8, 0.12, 34, INK, font="Segoe UI Semibold")

# 4. How it works
s = slide(GRAPHITE)
text(s, "How it works", 0.07, 0.10, 0.8, 0.2, 54, BONE)
for i, word in enumerate(["Plan", "Build", "Ship"]):
    shape(s, MSO_SHAPE.OVAL, 0.12 + i * 0.29, 0.42, 0.14, 0.249, [YELLOW, AQUA, CORAL][i])
    text(s, str(i + 1), 0.12 + i * 0.29, 0.455, 0.14, 0.2, 60, INK, align=PP_ALIGN.CENTER)
    text(s, word, 0.07 + i * 0.29, 0.72, 0.24, 0.12, 30, BONE, align=PP_ALIGN.CENTER)

# 5. Market
s = slide(AQUA)
text(s, "A market\nready to move", 0.07, 0.14, 0.8, 0.5, 66, INK)
for i, hgt in enumerate([0.18, 0.28, 0.40, 0.56]):
    shape(s, MSO_SHAPE.ROUNDED_RECTANGLE, 0.62 + i * 0.085, 0.86 - hgt, 0.065, hgt, INK)

# 6. Roadmap
s = slide(NIGHT)
text(s, "Roadmap", 0.07, 0.10, 0.8, 0.2, 54, BONE)
shape(s, MSO_SHAPE.RECTANGLE, 0.09, 0.545, 0.82, 0.012, "3A4046")
for i, (q, what) in enumerate([("Q1", "Prototype"), ("Q2", "Private beta"), ("Q3", "Launch"), ("Q4", "Scale")]):
    shape(s, MSO_SHAPE.OVAL, 0.105 + i * 0.25, 0.515, 0.04, 0.071, [YELLOW, AQUA, CORAL, VIOLET][i])
    text(s, q, 0.085 + i * 0.25, 0.35, 0.2, 0.12, 36, BONE)
    small(s, what, 0.087 + i * 0.25, 0.63, "DAD8D2", w=0.22)

# 7. Team
s = slide(CORAL)
text(s, "The team", 0.07, 0.10, 0.8, 0.2, 54, INK)
for i, name in enumerate(["Maya", "Jon", "Priya", "Sam"]):
    shape(s, MSO_SHAPE.OVAL, 0.09 + i * 0.215, 0.40, 0.13, 0.231, INK)
    text(s, name, 0.055 + i * 0.215, 0.69, 0.2, 0.12, 28, INK, align=PP_ALIGN.CENTER)

# 8. The ask
s = slide(VIOLET)
text(s, "The ask", 0.07, 0.10, 0.8, 0.2, 54, INK)
text(s, "$2M", 0.07, 0.30, 0.8, 0.4, 150, INK)
small(s, "to take Aurora to every team", 0.075, 0.76, INK)

# 9. Thank you
s = slide(BONE)
shape(s, MSO_SHAPE.ROUNDED_RECTANGLE, 0.07, 0.72, 0.16, 0.035, YELLOW)
text(s, "Thank you", 0.07, 0.30, 0.86, 0.3, 96, INK)
small(s, "Questions?", 0.075, 0.60, "5A6066")

out = Path(__file__).resolve().parent.parent / "assets" / "launch-plan.pptx"
prs.save(out)
print(out)
