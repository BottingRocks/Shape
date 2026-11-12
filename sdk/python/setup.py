"""Install the shape Python SDK: uv pip install --no-build-isolation ./sdk/python

Pure Python; the native library (libshape) is found at run time, or bundled
when a build script has copied it into shape/lib/ before installing.

There is deliberately no pyproject.toml: with one, older pip insists on a
PEP 517 build that needs the `wheel` package (absent offline). Without it,
pip uses the setuptools already installed and works with no network.
"""

from setuptools import setup

setup(
    name="shape",
    version="0.1.0",
    description="Python SDK of shape (ctypes binding of libshape)",
    license="GPL-3.0-only",
    license_files=["LICENSE"],
    packages=["shape"],
    package_data={"shape": ["py.typed", "lib/*.dylib", "lib/*.so", "lib/*.dll"]},
    python_requires=">=3.9",
    zip_safe=False,
)
