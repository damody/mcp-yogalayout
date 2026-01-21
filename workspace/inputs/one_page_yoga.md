# 導入 Anti-Lag SDK 可望降低手遊觸控延遲 5%
參考 AMD Anti-Lag 2 技術，在天璣平台實現引擎級同步

## 已驗證的成功要素

PC 平台（AMD Radeon）已驗證 Anti-Lag 2 可大幅降低延遲，關鍵在於三個設計：

- **解決 [[CPU-Bound]] 排隊問題**：當 CPU 處理速度快於 GPU 時，會在渲染管線中產生 [[Frame Queue]]（幀佇列），造成「排隊延遲」。Anti-Lag 2 透過限制佇列深度，確保 CPU 不會過度超前 GPU [C10]
- **引擎內同步點**：在遊戲引擎的渲染管線中（如 `Present()` 之前）插入同步點，這是從「驅動層」深入到「遊戲引擎內」的關鍵改變 [C10]
- **輸入採樣對齊**：確保滑鼠/鍵盤的輸入採樣發生在該幀渲染開始前的最後一刻，讓玩家操作對應的是「即將顯示」的畫面 [C10]
- **實測效果**：在競技類遊戲（如《CS2》）中，系統延遲降低約 37%，響應流暢度明顯提升 [C10]

## 現況與問題

天璣 9500 平台運行 Android 遊戲時，存在以下延遲瓶頸：

- **[[BufferQueue]] 機制的天生延遲**：Android 的生產者（App）與消費者（[[SurfaceFlinger]]）之間採用緩衝區輪轉機制，設計上就存在 1-2 幀的緩衝延遲，這是系統架構造成的限制 [C11]
- **[[FPSGO]] 治理範圍有限**：現有 FPSGO 擅長「維持穩定幀率」與「節省功耗」，但它無法感知遊戲引擎內部從「輸入事件」到「開始渲染」之間的時間間隔 [C11]
- **SoC 調度開銷**：高負載時，輸入事件處理與渲染執行緒的喚醒時機會產生抖動，直接反映在總系統延遲上 [C11]
- **Android 合成器全程參與**：不同於 PC 的 [[MPO]] 技術可讓遊戲畫面繞過合成器直接顯示，Android 的 SurfaceFlinger 幾乎總是參與畫面合成 [C21]

## 技術關鍵點 / 為何可能有效

Android 與 PC 的輸入到顯示路徑有高度相似性，因此 Anti-Lag 方法可能同樣有效：

- **路徑相似性**：兩者都經過「輸入捕獲 - 引擎處理 - GPU 渲染 - 系統合成 - 顯示輸出」的完整鏈路 [C12]
- **同步點可對應**：PC 的關鍵同步點是「CPU `Present()` 與 GPU 執行完畢」，Android 則是「遊戲引擎畫好一幀後提交給系統（RenderThread 提交），與系統合成器取用畫面準備顯示（SurfaceFlinger 消費）的時間點對齊」，概念一致 [C12]
- **SDK 整合路徑明確**：可透過 [[MAGT]]（MediaTek 遊戲優化框架）或 [[Performance Hint API]]（Android 系統資源預告介面）擴展來實現引擎與 SoC 的深度同步 [C12]
- **工程推論**：透過 SDK 實現引擎與 SoC 的深度節奏同步，能顯著降低「點擊到螢幕反應」的總延遲 [C13]


<fig id="appendix:4" ratio="3:1" kind="diagram" alt="展示 Unity 與 Unreal 引擎的渲染架構差異及其對 Anti-Lag 效果的影響" />


<fig id="appendix:1android" ratio="3:1" kind="diagram" alt="從觸控到顯示的完整四階段流程，標註每階段的關鍵元件" />

## 引擎架構差異（可能影響改善幅度）

不同遊戲引擎的架構設計會影響 Anti-Lag 技術的效果 [C8]：

- **Unity（低延遲模式）**：邏輯與渲染緊密耦合，緩衝區深度通常較淺（Double Buffering），從點擊到顯示的路徑較短，但容易受主執行緒波動影響
- **Unreal（高吞吐模式）**：採用 Pipelined 設計，渲染工作可能橫跨多個幀週期，GPU 利用率高但操作感可能存在微小滯後

**對 POC 的影響**：建議同時選擇 Unity 與 Unreal 引擎的遊戲進行測試，以評估不同架構下的改善幅度差異。

## 對照表

| 比較項目 | PC 平台 (AMD Anti-Lag 2) | 手機平台 (MTK Proposed) |
|----------|-------------------------|-------------------------|
| 同步層級 | In-engine SDK (Vulkan/DX12) | MAGT / Performance Hint API 擴展 |
| 關鍵同步點 | CPU `Present()` 與 GPU 執行完畢 | 遊戲引擎提交畫面（RenderThread）與系統取用畫面（SurfaceFlinger）的時間點對齊 |
| 量測工具 | [[FLM]] (Software/Hardware) | FLM + Android Systrace (AGI) |
| 合成繞過 | MPO 可繞過 DWM 直接顯示 | SurfaceFlinger 幾乎全程參與 |


<fig id="appendix:2pc_vs_android" ratio="3:1" kind="diagram" alt="對照 PC 與 Android 在各延遲階段的處理差異" />

## 預期效益

- **消除無效排隊**：當 GPU 忙碌時，主動通知引擎推遲下一幀的輸入採樣，確保玩家點擊對應的是「螢幕上最新的一幀」[C13]
- **優化渲染路徑**：縮短從「觸控事件觸發」到「核心渲染指令發出」的間隔，收斂 [[P99 Frame Time]] [C13]
- **預期改善幅度**：PC 平台實測可達 37%，但 Android 存在架構限制（如 SurfaceFlinger 無法繞過、BufferQueue 天生延遲），初步保守目標設定為 5% 以上，具體數據需待 POC 驗證


<fig id="main:antilag" ratio="3:1" kind="diagram" alt="展示導入 Anti-Lag SDK 前後，輸入到顯示延遲鏈路的差異" />

## POC 設計

**實驗條件：**
- A 組 (Baseline)：現行方案（僅開啟 FPSGO，無延遲同步）
- B 組 (Experimental)：導入 MTK Anti-Lag SDK（實現引擎與平台同步）

**量測工具：**
- [[FLM]] (Frame Latency Meter) 進行 End-to-End Latency 量測 [C14]
- 監控 Click-to-Photon（從點擊觸控到螢幕像素變化的總時間）
- 搭配 [[ftrace]] / [[LTR]] 追蹤系統執行流程

**遊戲場景：**
- 競技類手遊（射擊、動作類，對延遲敏感）
- 高幀率模式 (90/120 FPS)
- 建議選擇一款 Unity 遊戲、一款 Unreal 遊戲進行對比測試

## 成功判定準則

1. **系統延遲下降**
   全鏈路延遲在同等 FPS 下降低 5% 以上 [C14]

2. **[[1% Low]] 提升**
   Frame Time 抖動幅度明顯收斂，代表流暢度穩定 [C14]

3. **功耗行為**
   不因縮短延遲而造成溫升超標，維持功耗波動與 A 組持平 [C14]


<fig id="appendix:3" ratio="3:1" kind="diagram" alt="展示 Click-to-Photon 量測點與 POC 成功判定的三個維度" />

## 行動

建議立項進行 POC 驗證：整合 MAGT / Performance Hint API 擴展，在 1-2 款競技類手遊中實測 Anti-Lag 同步機制的效果。若驗證成功，可作為天璣平台的差異化賣點。
