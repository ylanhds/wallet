# ============================================
# 钱包服务 - 数据库迁移执行脚本
# ============================================

$host = "192.168.0.23"
$user = "dev"
$password = "dev9856AC"
$database = "zbs"
$sqlFile = "d:\projet\cargo\wallet-service\database\migration.sql"

Write-Host "=============================================" -ForegroundColor Cyan
Write-Host "钱包服务 - 数据库迁移" -ForegroundColor Cyan
Write-Host "=============================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "服务器：$host" -ForegroundColor Yellow
Write-Host "数据库：$database" -ForegroundColor Yellow
Write-Host "用户：$user" -ForegroundColor Yellow
Write-Host ""

# 读取 SQL 文件
Write-Host "读取 SQL 文件..." -ForegroundColor Green
$sqlContent = Get-Content $sqlFile -Raw -Encoding UTF8

# 使用 MySQL 命令行工具（如果存在）
$mysqlPath = "C:\Program Files\MySQL\MySQL Server 8.0\bin\mysql.exe"

if (Test-Path $mysqlPath) {
    Write-Host "找到 MySQL 客户端：$mysqlPath" -ForegroundColor Green
    & $mysqlPath -h $host -u $user -p$password $database -e $sqlContent
    
    if ($LASTEXITCODE -eq 0) {
        Write-Host ""
        Write-Host "✅ 迁移成功！" -ForegroundColor Green
        Write-Host ""
    } else {
        Write-Host ""
        Write-Host "❌ 迁移失败，错误代码：$LASTEXITCODE" -ForegroundColor Red
        Write-Host ""
    }
} else {
    Write-Host "⚠️  未找到 MySQL 命令行工具" -ForegroundColor Yellow
    Write-Host ""
    Write-Host "请手动执行以下命令：" -ForegroundColor Cyan
    Write-Host ""
    Write-Host "mysql -h $host -u $user -p$password $database < `"$sqlFile`"" -ForegroundColor White
    Write-Host ""
    Write-Host "或者使用 MySQL Workbench / Navicat 等工具打开并执行 migration.sql 文件" -ForegroundColor White
    Write-Host ""
}

Write-Host "按任意键退出..."
$null = $Host.UI.RawUI.ReadKey("NoEcho,IncludeKeyDown")
