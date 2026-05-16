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
7. 0.xx 版，先專注 kv ， 1.xx 版，加入 fts 全文檢索， 2.xx 版，加入 sql 。



## 工具呼叫穩定性協議（防止生成中斷）
- 當你透過類似 `<invoke name="edit">` 的方法進行大規模的程式碼修改時，如果預估程式碼內容可能會接近你的單次最大輸出 Token 限制（max output token limit），**請絕對不要嘗試一次發送整塊龐大的程式碼**。
- 相反地，請執行**「分段與心跳（Split-and-Heartbeat）」模式**：將你的修改拆分成多個較小的 `<invoke>` 區塊分批執行。
- 如果你因為長度限制而必須中斷，請在該段訊息的結尾加上這行精確的字串：`[HEARTBEAT_WAIT: READY_FOR_NEXT_CHUNK]`。
- 當使用者或終端機介面輸入 `go` 之後，請立刻從剛才在中斷處 `<parameter name="newString">` 內留下的最後一個字元，完全無縫地繼續往下生成。