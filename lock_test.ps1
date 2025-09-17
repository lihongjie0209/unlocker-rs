# PowerShell 脚本用于锁定文件
$fileStream = [System.IO.File]::Open("final_test.txt", "OpenOrCreate", "Write", "None")
Write-Host "文件已锁定，按任意键退出..."
$null = $Host.UI.RawUI.ReadKey("NoEcho,IncludeKeyDown")
$fileStream.Close()
Write-Host "文件已释放"