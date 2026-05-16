# db6 -- Unified database with pluggable storage engines (Memory/BTree/LSM) KV + SQL + FTS

1. 同時支援 LSM-tree, BTree 與 memory 記憶體模式。
    * 記憶體模式應該用什麼資料結構呢？（以 Page 為單位）
    * LSM-tree 參考 lsm6/
    * Btree 參考 btree6/ 
2. 包含 key-value 的 api 與 sql 的 api
3. 使用 LSM-tree 時，不支援那些不適合 LSM-tree 的 SQL 語法，例如 JOIN。
4. 使用 Btree 時，支援完整的 sqlite 版本的語法。 （參考 sql6/)
5. 支援全文檢索 (參考 sql6/，但是用 kv 介面來當基礎，最後支援 sql6 的全文檢索語法)
6. 記得讓 sql 介面，建立在 kv 的基礎上
    * 可以繼承 KvStore ，加入 SortedStore 這類的類別，然後銜接到 SQL