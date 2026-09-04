#!/usr/bin/env python3
"""生成 wavAnaly 图标: surfer/assets/wavanaly.png (256) / wavanaly.ico / logo.png / favicon.ico

图形: 深色圆角方块, 上面一条亮绿色数字波形 (方波 + 中间一个总线六边形), 右下角一个放大镜圆环。
"""
from PIL import Image, ImageDraw
import os

ASSETS = os.path.join(os.path.dirname(__file__), '..', 'surfer', 'assets')
S = 1024  # 超采样尺寸


def draw(size=S):
    im = Image.new('RGBA', (size, size), (0, 0, 0, 0))
    d = ImageDraw.Draw(im)
    u = size / 1024
    # 背景圆角方块 (与查看器深色主题接近)
    r = 200 * u
    d.rounded_rectangle([0, 0, size - 1, size - 1], radius=r, fill=(16, 27, 38, 255))
    # 内侧细边
    d.rounded_rectangle([18 * u, 18 * u, size - 18 * u, size - 18 * u], radius=r - 18 * u,
                        outline=(38, 62, 80, 255), width=int(10 * u))

    green = (126, 217, 87, 255)
    amber = (255, 196, 61, 255)
    w = int(54 * u)

    # 方波: 左段
    y_hi, y_lo = 330 * u, 560 * u
    pts = [(120 * u, y_lo), (250 * u, y_lo), (250 * u, y_hi), (380 * u, y_hi), (380 * u, y_lo), (470 * u, y_lo)]
    d.line(pts, fill=green, width=w, joint='curve')
    # 总线六边形 (中间)
    hx0, hx1 = 470 * u, 760 * u
    hy0, hy1 = y_hi, y_lo
    k = 55 * u
    hexagon = [(hx0, (hy0 + hy1) / 2), (hx0 + k, hy0), (hx1 - k, hy0), (hx1, (hy0 + hy1) / 2), (hx1 - k, hy1), (hx0 + k, hy1)]
    d.polygon(hexagon, outline=amber, width=w)
    # 右段
    d.line([(hx1, (hy0 + hy1) / 2), (830 * u, (hy0 + hy1) / 2), (830 * u, y_hi), (900 * u, y_hi)], fill=green, width=w, joint='curve')

    # 第二条: 时钟 (下方, 细一些)
    w2 = int(40 * u)
    y2h, y2l = 660 * u, 800 * u
    x = 120 * u
    clk = [(x, y2l)]
    step = 97.5 * u
    for i in range(8):
        x2 = x + step
        clk += [(x, y2h if i % 2 == 0 else y2l), (x2, y2h if i % 2 == 0 else y2l)]
        x = x2
    d.line(clk, fill=(96, 165, 250, 255), width=w2, joint='curve')

    # 放大镜 (右下)
    cx, cy, rr = 790 * u, 760 * u, 105 * u
    d.ellipse([cx - rr, cy - rr, cx + rr, cy + rr], fill=(16, 27, 38, 230), outline=(235, 240, 245, 255), width=int(34 * u))
    d.line([(cx + rr * 0.7, cy + rr * 0.7), (cx + rr * 1.55, cy + rr * 1.55)], fill=(235, 240, 245, 255), width=int(52 * u))
    return im


def main():
    big = draw()
    os.makedirs(ASSETS, exist_ok=True)
    png256 = big.resize((256, 256), Image.LANCZOS)
    png256.save(os.path.join(ASSETS, 'wavanaly.png'))
    big.resize((512, 512), Image.LANCZOS).save(os.path.join(ASSETS, 'logo.png'))
    sizes = [(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)]
    png256.save(os.path.join(ASSETS, 'wavanaly.ico'), sizes=sizes)
    png256.save(os.path.join(ASSETS, 'favicon.ico'), sizes=[(16, 16), (32, 32), (48, 48)])
    big.save(os.path.join(os.path.dirname(__file__), '..', 'docs', 'wavanaly_icon_1024.png'))
    print('icons written to', os.path.abspath(ASSETS))


if __name__ == '__main__':
    main()
