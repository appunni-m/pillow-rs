"""Test configuration for pillow-rs."""
import pytest
import yaml
from pathlib import Path


def pytest_addoption(parser):
    parser.addoption("--pil", action="store_true", default=False,
                     help="Run tests against Pillow instead of pillow-rs")
    parser.addoption("--manifest", action="store", default="manifest.yaml",
                     help="Path to manifest.yaml")


@pytest.fixture(scope="session")
def manifest(request):
    manifest_path = Path(request.config.getoption("--manifest"))
    with open(manifest_path) as f:
        return yaml.safe_load(f)


@pytest.fixture(scope="session")
def use_pillow(request):
    return request.config.getoption("--pil", False)


@pytest.fixture(scope="session")
def ImageModule(use_pillow):
    if use_pillow:
        from PIL import Image as PILImage
        return PILImage
    else:
        from pillow_rs import Image
        return Image


@pytest.fixture
def Image(ImageModule):
    return ImageModule


def pytest_collection_modifyitems(config, items):
    if config.getoption("--pil", False):
        skip_rs = pytest.mark.skip(reason="requires pillow-rs")
        for item in items:
            if "rs_only" in item.keywords:
                item.add_marker(skip_rs)


def pytest_configure(config):
    config.addinivalue_line("markers",
        "covers(func, mode=None, variant=None): mark test as covering a manifest entry")
    config.addinivalue_line("markers",
        "rs_only: test only applies to pillow-rs (not Pillow)")
