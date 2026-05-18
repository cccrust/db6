import { describe, test, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert';
import { Client } from '../src/index.js';

const DB_URL = 'http://localhost:50052';

describe('db6js', () => {
  let db;

  beforeEach(() => {
    db = new Client(DB_URL);
  });

  afterEach(async () => {
    await db.close();
  });

  test('health', async () => {
    const result = await db.health();
    assert.strictEqual(result, true);
  });

  test('put and get', async () => {
    await db.put(1, 'test_key', 'test_value');
    const [value, found] = await db.get(1, 'test_key');
    assert.strictEqual(found, true);
    assert.strictEqual(value, 'test_value');
  });

  test('delete', async () => {
    await db.put(1, 'delete_key', 'delete_value');
    await db.delete(1, 'delete_key');
    const [, found] = await db.get(1, 'delete_key');
    assert.strictEqual(found, false);
  });

  test('get not found', async () => {
    const [, found] = await db.get(1, 'nonexistent_key');
    assert.strictEqual(found, false);
  });

  test('batch put', async () => {
    await db.batchPut(1, [['batch1', 'v1'], ['batch2', 'v2']]);
    const [v1] = await db.get(1, 'batch1');
    const [v2] = await db.get(1, 'batch2');
    assert.strictEqual(v1, 'v1');
    assert.strictEqual(v2, 'v2');
  });

  test('scan', async () => {
    await db.batchPut(1, [['scan_a', '1'], ['scan_b', '2'], ['scan_c', '3']]);
    const results = await db.scan(1, 'scan_', 'scan_\xff');
    const keys = results.map(([k]) => k).sort();
    assert.ok(keys.includes('scan_a'));
    assert.ok(keys.includes('scan_b'));
    assert.ok(keys.includes('scan_c'));
  });

  test('stats', async () => {
    const stats = await db.stats();
    assert.ok('key_count' in stats);
    assert.ok('engine' in stats);
    assert.ok('size_bytes' in stats);
  });
});