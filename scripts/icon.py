"""Пиксельная хлопушка — значок приложения и сайта.

Рисует хлопушку на сетке 32×32 (как пиксель-арт: ступенчатые края, чёрный контур, блики)
и сохраняет SVG из прямоугольников: desktop/icon-src/icon.svg (на плитке, для приложения),
web/static/favicon.svg и web/static/logo.svg (без плитки). PNG — `python3 scripts/icon.py png`
(нужен Pillow): icon-src/icon.png 1024, web/static/icon-192/512, apple-touch-icon.
"""

import math
import sys
from pathlib import Path

N = 32
ROOT = Path(__file__).resolve().parent.parent

INK = "#111111"      # контур
BODY = "#3d3d3f"     # доска
BODY_HI = "#56565a"  # светлая грань доски
LINE = "#1d1d1f"     # строки на доске
STRIPE = "#f4f4f5"   # белые полосы
STRIPE_SH = "#b8b8bc"
SHADOW = "#2a2a2c"


def rot(x, y, cx, cy, a):
    s, c = math.sin(a), math.cos(a)
    x, y = x - cx, y - cy
    return cx + x * c - y * s, cy + x * s + y * c


def grid():
    g = [[None] * N for _ in range(N)]
    tilt = math.radians(-12)  # вся хлопушка чуть наклонена, как на картинке
    open_ = math.radians(-13)  # верхняя планка приоткрыта
    hx, hy = 7.0, 14.0  # шарнир — левый верхний угол доски
    for j in range(N):
        for i in range(N):
            # Точка в системе хлопушки (без общего наклона).
            x, y = rot(i + 0.5, j + 0.5, 16, 17, -tilt)
            # Доска: x 7..27, y 14..27; верхняя полоса 14..18 — нижняя планка.
            if 7 <= x < 27 and 14 <= y < 27.5:
                if y < 18:
                    k = int((x - y * 0.9) // 2.6)
                    g[j][i] = STRIPE if k % 2 == 0 else BODY
                else:
                    g[j][i] = BODY
                    if 7 <= x < 9:
                        g[j][i] = BODY_HI
                    if int(y) in (21, 24) and 11 <= x < 25 and int(x) % 6 != 0:
                        g[j][i] = LINE
                continue
            # Верхняя планка: поворачивается вокруг шарнира.
            u, v = rot(x, y, hx, hy, -open_)
            if 7 <= u < 27.5 and 9.6 <= v < 13.6:
                k = int((u + v * 0.9) // 2.6)
                g[j][i] = STRIPE if k % 2 == 0 else BODY
                if v >= 12.8 and g[j][i] == STRIPE:
                    g[j][i] = STRIPE_SH
    # Контур: пустая клетка рядом с закрашенной.
    filled = {(i, j) for j in range(N) for i in range(N) if g[j][i]}
    out = [row[:] for row in g]
    for j in range(N):
        for i in range(N):
            if g[j][i] is None and any((i + di, j + dj) in filled for di, dj in ((1, 0), (-1, 0), (0, 1), (0, -1))):
                out[j][i] = INK
    # Тень вправо-вниз — объём, как на картинке.
    shaded = {(i, j) for j in range(N) for i in range(N) if out[j][i]}
    for j in range(N):
        for i in range(N):
            if out[j][i] is None and (i - 1, j - 1) in shaded and (i - 1, j) in shaded:
                out[j][i] = INK
    return out


def rects(g, scale, ox, oy):
    parts = []
    for j, row in enumerate(g):
        i = 0
        while i < N:
            c = row[i]
            if c is None:
                i += 1
                continue
            k = i
            while k < N and row[k] == c:
                k += 1
            parts.append(f'<rect x="{ox + i * scale:g}" y="{oy + j * scale:g}" width="{(k - i) * scale:g}" height="{scale:g}" fill="{c}"/>')
            i = k
    return "".join(parts)


def svg(g, tile):
    if tile:
        # Плитка по сетке macOS: 824 из 1024, скругление 185.
        body = rects(g, 24, 128, 112)
        return (
            '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1024 1024" shape-rendering="crispEdges">'
            '<rect x="100" y="100" width="824" height="824" rx="185" fill="#d6d6d9" shape-rendering="geometricPrecision"/>'
            f"{body}</svg>"
        )
    return f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {N} {N}" shape-rendering="crispEdges">{rects(g, 1, 0, 0)}</svg>'


def png(g, size, tile):
    from PIL import Image, ImageDraw

    big = 1024
    im = Image.new("RGBA", (big, big), (0, 0, 0, 0))
    d = ImageDraw.Draw(im)
    if tile:
        d.rounded_rectangle((100, 100, 923, 923), radius=185, fill="#d6d6d9")
        scale, ox, oy = 24, 128, 112
    else:
        scale, ox, oy = 32, 0, 0
    for j, row in enumerate(g):
        for i, c in enumerate(row):
            if c:
                d.rectangle((ox + i * scale, oy + j * scale, ox + (i + 1) * scale - 1, oy + (j + 1) * scale - 1), fill=c)
    return im.resize((size, size), Image.LANCZOS) if size != big else im


def main():
    g = grid()
    (ROOT / "desktop/icon-src/icon.svg").write_text(svg(g, True))
    (ROOT / "web/static/favicon.svg").write_text(svg(g, True))
    (ROOT / "web/static/logo.svg").write_text(svg(g, False))
    if "png" in sys.argv:
        png(g, 1024, True).save(ROOT / "desktop/icon-src/icon.png")
        for s in (192, 512):
            png(g, s, True).save(ROOT / f"web/static/icon-{s}.png")
        # iPhone сам скругляет углы — плитка без прозрачности.
        from PIL import Image

        t = png(g, 1024, False).resize((150, 150), Image.NEAREST)
        a = Image.new("RGB", (180, 180), "#d6d6d9")
        a.paste(t, (15, 12), t)
        a.save(ROOT / "web/static/apple-touch-icon.png")


if __name__ == "__main__":
    main()
