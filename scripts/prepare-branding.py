from pathlib import Path

from PIL import Image, ImageOps


root = Path(__file__).resolve().parent.parent / "assets" / "branding"
mark = Image.open(root / "mskey-mark.png").convert("RGBA")
logo = Image.open(root / "mskey-logo.png").convert("RGBA")
icon = ImageOps.contain(mark, (1024, 1024), Image.Resampling.LANCZOS)
icon.save(root / "mskey.ico", sizes=[(size, size) for size in (16, 20, 24, 32, 40, 48, 64, 128, 256)])
icon.save(root / "mskey.icns")
for size in (16, 24, 32, 48, 64, 128, 256, 512):
    icon.resize((size, size), Image.Resampling.LANCZOS).save(root / f"mskey-{size}.png")
for name, size, bounds in (
    ("installer-header.bmp", (150, 57), (52, 52)),
    ("installer-sidebar.bmp", (164, 314), (148, 150)),
):
    canvas = Image.new("RGB", size, "white")
    image = ImageOps.contain(logo, bounds, Image.Resampling.LANCZOS)
    canvas.paste(image, ((size[0] - image.width) // 2, (size[1] - image.height) // 2), image)
    canvas.save(root / name)
