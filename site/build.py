#!/usr/bin/env python3
"""Build the pigmentlab.net pages.

The site is two self-contained HTML files. Everything — fonts, the icon, the
shared stylesheet — is inlined, because the pages deploy as plain static files
and must not depend on any other host.

    python3 site/build.py            # writes docs/index.html and docs/guide.html

Sources:
    site/theme.css              shared design system (both pages)
    site/index.template.html    the landing page
    site/guide.template.html    the documentation
    site/fonts/*.woff2          JetBrains Mono, subset to latin + box drawing

Edit the templates, re-run this, commit the result. `docs/` is what ships.
"""
import base64
import pathlib
import sys

HERE = pathlib.Path(__file__).resolve().parent
REPO = HERE.parent
FONTS = HERE / "fonts"
OUT = REPO / "docs"

PAGES = [
    ("index.template.html", "index.html"),
    ("guide.template.html", "guide.html"),
]


def data_uri(path: pathlib.Path, mime: str) -> str:
    return f"data:{mime};base64," + base64.b64encode(path.read_bytes()).decode("ascii")


def tokens() -> dict:
    return {
        "__FONT_JB__": data_uri(FONTS / "JBMono-Regular.sub.woff2", "font/woff2"),
        "__FONT_JB_BOLD__": data_uri(FONTS / "JBMono-Bold.sub.woff2", "font/woff2"),
        "__ICON__": data_uri(
            REPO / "packaging/icons/128x128/net.pigmentlab.Pigment.png", "image/png"
        ),
    }


def build(src: pathlib.Path, dst: pathlib.Path, subs: dict) -> None:
    html = src.read_text()
    # __THEME__ first: the stylesheet itself contains font and icon tokens.
    html = html.replace("__THEME__", (HERE / "theme.css").read_text())
    for token, value in subs.items():
        html = html.replace(token, value)
    leftover = [t for t in list(subs) + ["__THEME__"] if t in html]
    if leftover:
        raise SystemExit(f"{src.name}: unsubstituted tokens: {leftover}")
    dst.write_text(html)
    print(f"  {dst.relative_to(REPO)}  {len(html.encode()) / 1024:.0f} KB")


def main() -> int:
    subs = tokens()
    print("Building pigmentlab.net:")
    for src_name, dst_name in PAGES:
        build(HERE / src_name, OUT / dst_name, subs)
    return 0


if __name__ == "__main__":
    sys.exit(main())
