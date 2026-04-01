@echo off
echo ========================================
echo 🧪 钱包服务新功能测试脚本
echo ========================================
echo.

set API_BASE=http://127.0.0.1:3000

echo [1/6] 测试健康检查...
curl -s %API_BASE%/health | findstr /v "^$"
echo.
echo.

echo [2/6] 批量创建 5 个钱包...
curl -X POST -H "Content-Type: application/json" -d "{\"count\":5}" %API_BASE%/wallets/batch
echo.
echo.

echo [3/6] 获取钱包列表...
curl -s %API_BASE%/wallets
echo.
echo.

echo [4/6] 搜索钱包...
curl -s "%API_BASE%/wallets/search?q=0x"
echo.
echo.

echo [5/6] 导出所有钱包...
curl -s %API_BASE%/wallets/export > wallets_export.json
echo ✅ 已导出到 wallets_export.json
echo.

echo [6/6] 查看导出的文件前 20 行...
type wallets_export.json | findstr /n "^" | findstr /c:"[1]" /c:"[2]" /c:"[3]" /c:"[4]" /c:"[5]" /c:"[6]" /c:"[7]" /c:"[8]" /c:"[9]" /c:"[10]" /c:"[11]" /c:"[12]" /c:"[13]" /c:"[14]" /c:"[15]" /c:"[16]" /c:"[17]" /c:"[18]" /c:"[19]" /c:"[20]"
echo.

echo ========================================
echo ✅ 测试完成！
echo ========================================
echo.
echo 提示：
echo - 删除钱包：curl -X DELETE %API_BASE%/wallets/{address}
echo - 导入助记词：curl -X POST -H "Content-Type: application/json" -d "{\"mnemonic\":\"your 12 words...\"}" %API_BASE%/wallets/import
echo.
