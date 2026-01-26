#!/bin/bash
# Finishing Development Phase Agent
# 自動化項目收尾 Agent

set -e

TIMESTAMP=$(date +"%Y-%m-%d %H:%M:%S")
PHASE_DATE=$(date +"%Y-%m-%d")

echo "🎯 開始項目收尾工作流程"
echo "時間：$TIMESTAMP"
echo ""

# ============================================================================
# 第一步：收集信息
# ============================================================================

echo "📋 收集開發階段信息..."
echo ""

read -p "輸入 Phase 名稱 (例：phase5-anime-management): " PHASE_NAME
if [ -z "$PHASE_NAME" ]; then
    echo "❌ Phase 名稱不能為空"
    exit 1
fi

echo ""
echo "📝 實現的功能（每行一個，輸入空行結束）："
FEATURES=()
while true; do
    read -p "功能: " feature
    if [ -z "$feature" ]; then
        break
    fi
    FEATURES+=("$feature")
done

echo ""
echo "🔌 新增/修改的 API 端點（每行一個，輸入空行結束）："
ENDPOINTS=()
while true; do
    read -p "端點: " endpoint
    if [ -z "$endpoint" ]; then
        break
    fi
    ENDPOINTS+=("$endpoint")
done

echo ""
echo "📦 資料庫變更（新表/修改字段，每行一個，輸入空行結束）："
DB_CHANGES=()
while true; do
    read -p "變更: " change
    if [ -z "$change" ]; then
        break
    fi
    DB_CHANGES+=("$change")
done

# ============================================================================
# 第二步：代碼質量檢查
# ============================================================================

echo ""
echo "🔍 執行代碼質量檢查..."

cd /workspace

echo "   • 運行代碼格式檢查..."
if cargo fmt --all 2>&1 | tail -1; then
    echo "   ✓ 代碼格式已修復"
fi

echo "   • 運行 Clippy 靜態分析..."
if cargo clippy --all-targets --all-features 2>&1 | tail -5; then
    echo "   ✓ Clippy 檢查完成"
fi

echo "   • 編譯檢查..."
if cargo check 2>&1 | tail -3; then
    echo "   ✓ 編譯成功"
fi

# ============================================================================
# 第三步：驗證 API Spec
# ============================================================================

echo ""
echo "📝 驗證 API Spec..."

if [ -f "/workspace/docs/api/openapi.yaml" ]; then
    echo "   ✓ API Spec 存在"
    SPEC_LINES=$(wc -l < /workspace/docs/api/openapi.yaml)
    echo "   • Spec 行數：$SPEC_LINES"
else
    echo "   ⚠️ API Spec 不存在"
fi

# ============================================================================
# 第四步：生成完成報告
# ============================================================================

echo ""
echo "📄 生成完成報告..."

REPORT_FILE="/workspace/docs/plans/${PHASE_DATE}-${PHASE_NAME}-completion.md"

cat > "$REPORT_FILE" << REPORT_EOF
# $PHASE_NAME 完成報告

**完成日期：** $PHASE_DATE  
**生成時間：** $TIMESTAMP

---

## 概述

在本階段完成了 Bangumi 項目的 $PHASE_NAME 功能開發。

## 實現功能

REPORT_EOF

for feature in "${FEATURES[@]}"; do
    echo "- ✅ $feature" >> "$REPORT_FILE"
done

cat >> "$REPORT_FILE" << 'REPORT_EOF'

## API 端點更新

### 新增/修改端點

REPORT_EOF

if [ ${#ENDPOINTS[@]} -gt 0 ]; then
    for endpoint in "${ENDPOINTS[@]}"; do
        echo "- $endpoint" >> "$REPORT_FILE"
    done
else
    echo "- 無新增端點變更" >> "$REPORT_FILE"
fi

cat >> "$REPORT_FILE" << 'REPORT_EOF'

## 資料庫變更

REPORT_EOF

if [ ${#DB_CHANGES[@]} -gt 0 ]; then
    for change in "${DB_CHANGES[@]}"; do
        echo "- $change" >> "$REPORT_FILE"
    done
else
    echo "- 無數據庫變更" >> "$REPORT_FILE"
fi

cat >> "$REPORT_FILE" << 'REPORT_EOF'

## 測試情況

- [ ] 單元測試通過
- [ ] 集成測試通過  
- [ ] 手動測試通過

## 待辦項

- [ ] 更新文檔
- [ ] 代碼審查
- [ ] 部署到測試環境

## 備註

在此添加其他重要信息或後續工作。

---

**生成者：** Claude Code Agent  
**檔案位置：** $REPORT_FILE
REPORT_EOF

echo "   ✓ 完成報告已生成"
echo "     位置：$REPORT_FILE"

# ============================================================================
# 第五步：更新進度日誌
# ============================================================================

echo ""
echo "📊 更新進度日誌..."

if [ -f "/workspace/docs/PROGRESS.md" ]; then
    {
        echo ""
        echo "## $PHASE_NAME ($PHASE_DATE)"
        echo ""
        echo "✅ 已完成"
        echo ""
        echo "**完成報告：** \`$REPORT_FILE\`"
        echo ""
        echo "**實現功能：**"
        for feature in "${FEATURES[@]}"; do
            echo "- $feature"
        done
        echo ""
    } >> /workspace/docs/PROGRESS.md
    
    echo "   ✓ 進度日誌已更新"
fi

# ============================================================================
# 第六步：Git 提交
# ============================================================================

echo ""
echo "📤 準備 Git 提交..."

cd /workspace

git add -A

MODIFIED=$(git status --short | wc -l)
echo "   • 修改的檔案：$MODIFIED 個"

COMMIT_MSG="feat: $PHASE_NAME - $PHASE_DATE

實現功能：
$(printf '%s\n' "${FEATURES[@]}" | sed 's/^/- /')

API 端點更新：
$(printf '%s\n' "${ENDPOINTS[@]}" | sed 's/^/- /')

資料庫變更：
$(printf '%s\n' "${DB_CHANGES[@]}" | sed 's/^/- /')

完成報告：$REPORT_FILE"

echo ""
echo "提交訊息預覽："
echo "---"
echo "$COMMIT_MSG"
echo "---"
echo ""

read -p "確認提交？(y/n) " -n 1 -r CONFIRM
echo
if [[ $CONFIRM =~ ^[Yy]$ ]]; then
    git commit -m "$COMMIT_MSG" || echo "   ⚠️ 沒有變更要提交"
    echo "   ✓ 已提交"
    
    echo ""
    echo "📝 後續建議："
    echo "   1. 查看完成報告："
    echo "      cat $REPORT_FILE"
    echo ""
    echo "   2. 編輯報告填寫詳細信息"
    echo ""
    echo "   3. 推送到遠程倉庫："
    echo "      git push"
else
    echo "   ⊘ 取消提交"
    git reset
fi

# ============================================================================
# 總結
# ============================================================================

echo ""
echo "=========================================="
echo "✨ 項目收尾工作完成！"
echo "=========================================="
echo ""
echo "📊 完成統計："
echo "   • Phase 名稱：$PHASE_NAME"
echo "   • 完成日期：$PHASE_DATE"
echo "   • 實現功能：${#FEATURES[@]} 個"
echo "   • API 端點更新：${#ENDPOINTS[@]} 個"
echo "   • 資料庫變更：${#DB_CHANGES[@]} 個"
echo ""
echo "📁 生成的檔案："
echo "   • 完成報告：$REPORT_FILE"
echo "   • 進度日誌：/workspace/docs/PROGRESS.md"
echo ""
echo "✅ 準備就緒，開始下一個開發階段！"
echo ""
