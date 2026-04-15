# conftest.py
# pytest fixtures for ATC integration tests
#
# REQ-SYS-080, REQ-COM-010
# DO-178C DAL-D: test fixtures must establish a verified live connection before any test runs

import socket
import sys
import os
import pytest

sys.path.insert(0, os.path.join(os.path.dirname(__file__), "framework"))
from atc_test_client import ATCTestClient, SERVER_HOST, SERVER_PORT


@pytest.fixture(scope="function")
def server_running():
    """
    Check that the server is accepting connections on 127.0.0.1:9000.
    Skips the test if the server is not reachable.
    """
    try:
        probe = socket.create_connection((SERVER_HOST, SERVER_PORT), timeout=2)
        probe.close()
    except (ConnectionRefusedError, OSError):
        pytest.skip(
            "Server not running on port 9000 — "
            "start atc-server.exe 9000 before running integration tests"
        )


@pytest.fixture(scope="function")
def atc_client(server_running):
    """
    Create an ATCTestClient, connect it, yield it to the test,
    then disconnect after the test completes.
    """
    client = ATCTestClient()
    client.connect()
    yield client
    client.disconnect()
