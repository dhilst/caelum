project = "Caelum"
copyright = "2025, Daniel Hilst"
author = "Daniel Hilst"

extensions = [
    "myst_parser",
]

templates_path = ["_templates"]
exclude_patterns = ["_build", "Thumbs.db", ".DS_Store"]

html_theme = "furo"
html_static_path = ["_static"]
html_title = "Caelum"

source_suffix = {
    ".rst": "restructuredtext",
    ".md": "markdown",
}
