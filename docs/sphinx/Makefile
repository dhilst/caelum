.PHONY: html clean

SPHINXBUILD = sphinx-build
SOURCEDIR = source
BUILDDIR = build

html:
	uv run $(SPHINXBUILD) -b html $(SOURCEDIR) $(BUILDDIR)/html

clean:
	rm -rf $(BUILDDIR)
