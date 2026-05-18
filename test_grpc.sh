#!/bin/bash
set -e

PORT=50051
HOST="127.0.0.1"
BASE_URL="http://$HOST:$PORT"

cleanup() {
    echo "Stopping server..."
    kill $SERVER_PID 2>/dev/null || true
}
trap cleanup EXIT

echo "Building grpc-server..."
cargo build --bin grpc-server

echo "Starting server..."
cargo run --bin grpc-server &
SERVER_PID=$!
sleep 2

echo ""
echo "=== Test 1: Put a key ==="
curl -s -X POST "$BASE_URL/kv/put" \
    -H "Content-Type: application/json" \
    -d '{"table_id":1,"key":"testkey","value":"testvalue"}'
echo ""

echo ""
echo "=== Test 2: Get the key ==="
curl -s -X POST "$BASE_URL/kv/get" \
    -H "Content-Type: application/json" \
    -d '{"table_id":1,"key":"testkey"}'
echo ""

echo ""
echo "=== Test 3: Put multiple keys ==="
curl -s -X POST "$BASE_URL/kv/batch_put" \
    -H "Content-Type: application/json" \
    -d '{"table_id":1,"pairs":[{"key":"key1","value":"value1"},{"key":"key2","value":"value2"},{"key":"key3","value":"value3"}]}'
echo ""

echo ""
echo "=== Test 4: Scan keys ==="
curl -s -X POST "$BASE_URL/kv/scan" \
    -H "Content-Type: application/json" \
    -d '{"table_id":1,"start":"key1","end":"key9"}'
echo ""

echo ""
echo "=== Test 5: Delete a key ==="
curl -s -X POST "$BASE_URL/kv/delete" \
    -H "Content-Type: application/json" \
    -d '{"table_id":1,"key":"testkey"}'
echo ""

echo ""
echo "=== Test 6: Get stats ==="
curl -s "$BASE_URL/kv/stats"
echo ""

echo ""
echo "=== All tests completed ==="