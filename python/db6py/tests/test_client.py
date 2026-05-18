"""pytest for db6py"""

import pytest
import sys
import os

sys.path.insert(0, os.path.join(os.path.dirname(__file__), "..", "src"))

from db6py import Client


@pytest.fixture
def db():
    return Client("http://localhost:50052")


class TestHealth:
    def test_health(self, db):
        assert db.health() == True


class TestKVOperations:
    def test_put_get(self, db):
        db.put(1, "test_key", "test_value")
        value, found = db.get(1, "test_key")
        assert found == True
        assert value == "test_value"

    def test_delete(self, db):
        db.put(1, "delete_key", "delete_value")
        db.delete(1, "delete_key")
        _, found = db.get(1, "delete_key")
        assert found == False

    def test_get_not_found(self, db):
        _, found = db.get(1, "nonexistent_key")
        assert found == False


class TestBatchOperations:
    def test_batch_put(self, db):
        db.batch_put(1, [("batch1", "v1"), ("batch2", "v2")])
        v1, _ = db.get(1, "batch1")
        v2, _ = db.get(1, "batch2")
        assert v1 == "v1"
        assert v2 == "v2"

    def test_scan(self, db):
        db.batch_put(1, [("scan_a", "1"), ("scan_b", "2"), ("scan_c", "3")])
        results = db.scan(1, "scan_", "scan_\xff")
        keys = sorted([k for k, v in results])
        assert "scan_a" in keys
        assert "scan_b" in keys
        assert "scan_c" in keys


class TestStats:
    def test_stats(self, db):
        stats = db.stats()
        assert "key_count" in stats
        assert "engine" in stats
        assert "size_bytes" in stats