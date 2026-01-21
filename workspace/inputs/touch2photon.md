# 導入 Touch-to-Photon 延遲優化技術，預估可降低手遊操作延遲 5% 以上

> PC 平台已驗證 Anti-Lag 技術可降低 37% 延遲，建議在 Dimensity 平台導入類似方案

## PC 平台已驗證的成功要素 [C7]

- 解決 CPU-Bound 排隊問題：CPU 太快累積過時畫面
- Present() 前插入同步點：控制 CPU 不超前
- 輸入採樣對齊到渲染前最後一刻
- CS2 實測延遲降低 37%
- Windows 11: MPO 跳過 DWM + VRR

## Dimensity + Android 現況問題 [C8][C9]

- BufferQueue 天生 1-2 幀緩衝延遲
- FPSGO 無法感知引擎內部輸入-渲染間隔
- 觸控採樣(240-480Hz)與顯示刷新同步困難
- SoC 調度開銷導致輸入事件抖動

## 技術關鍵點 / 為何可能有效 [C1][C10]

- 延遲路徑結構相似：輸入→引擎→提交→合成→顯示
- 可透過 MAGT / Performance Hint API 實現同步點
- 消除無效排隊：SDK 通知引擎「GPU 還在忙」
- 縮短輸入到渲染間隔：推遲採樣到渲染前一刻
- 收斂 P99 Frame Time / 1% Low 長尾延遲

<fig id="before_after" ratio="4:3" kind="diagram" alt="Anti-Lag 技術前後延遲對比：Before 輸入早採樣+BufferQueue 2-3幀；After 輸入晚採樣+BufferQueue 1幀" />

## 預期效益 [C8][C10]

- 全鏈路延遲降低 5%+ (保守目標)
- 1% Low FPS 提升，減少長尾卡頓
- Frame Time 抖動收斂
- 與 FPSGO 互補：SDK 管延遲、FPSGO 管幀率

## 引擎差異補充 [C3][C6]

- Unity：雙緩衝，單執行緒(UnityMain)，路徑短但易卡頓
- Unreal：流水線(Game→Render→RHI)，延遲長 1-2 幀

## 風險與緩解

- 風險：遊戲引擎 SDK 整合難度可能超出預期
- 緩解：優先與配合度高的遊戲廠商合作

## 平台對照表 [C9]

| 延遲階段 | PC (Anti-Lag 2) | Dimensity (建議) |
|----------|-----------------|------------------|
| 同步層級 | In-engine SDK | MAGT / Hint API |
| 關鍵同步點 | CPU Present() | RenderThread 提交 |
| Queue 控制 | Flip Queue = 1 | BufferQueue 控制 |
| 量測工具 | FLM | FLM + Systrace |
| 輸入採樣 | 滑鼠 1000Hz+ | 觸控 + InputDispatcher |

## POC 設計 [C11]

- A 組 (Baseline)：僅 FPSGO
- B 組 (Experimental)：FPSGO + Anti-Lag SDK
- 測試遊戲：PUBG Mobile / 傳說對決 / 原神
- 主要量測：FLM + Systrace (毫秒級)
- 輔助驗證：240fps 高速攝影機
- 樣本數：至少 1000 幀統計分析

## 成功判定準則 [C11]

- Click-to-Photon 延遲 -5% 以上
- 1% Low FPS >= A 組（不犧牲流暢度）
- Frame Time 標準差 <= A 組（抖動收斂）
- 功耗 <= A 組 +5%（不暴力提頻）

## 行動建議

建議決策：核准執行 POC 驗證

**時程規劃（7週）**：
- W1-2：SDK 原型開發 + Unity 引擎整合
- W3-4：Unreal 引擎整合 + 基礎測試
- W5-6：三款遊戲 POC 測試與數據收集
- W7：數據分析與結果報告

**所需資源**：
- 1 位資深 Android Framework 工程師
- 1 位遊戲引擎整合工程師
- 測試設備：Dimensity 9300 開發板 x2
- 量測設備：FLM 授權、高速攝影機
