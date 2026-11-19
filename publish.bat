@echo off
setlocal EnableExtensions DisableDelayedExpansion
title dkide Release 发布脚本
cd /d "%~dp0"

echo.
echo ============================================
echo dkide Release 发布脚本
echo ============================================

where node >nul 2>nul
if errorlevel 1 (
  echo [错误] 未找到 Node.js，请先安装 Node.js。
  goto :fail
)
where npm >nul 2>nul
if errorlevel 1 (
  echo [错误] 未找到 npm，请先安装 Node.js。
  goto :fail
)

set "DEFAULT_VERSION="
for /f "usebackq delims=" %%V in (`node -e "process.stdout.write(require('./package.json').version)"`) do set "DEFAULT_VERSION=%%V"
if not defined DEFAULT_VERSION set "DEFAULT_VERSION=1.1.0"

set "VERSION="
set "SKIP_PAUSE=0"
if not "%~1"=="" (
  set "VERSION=%~1"
  set "SKIP_PAUSE=1"
) else (
  set /p "VERSION=请输入版本号（默认 %DEFAULT_VERSION%，格式 x.y.z）："
)
if not defined VERSION set "VERSION=%DEFAULT_VERSION%"

node -e "if(!/^\d+\.\d+\.\d+$/.test(process.argv[1]))process.exit(1)" "%VERSION%"
if errorlevel 1 (
  echo [错误] 版本号格式不正确：%VERSION%，应为 x.y.z，例如 1.1.1。
  goto :fail
)

echo.
echo [1/4] 更新版本号到 %VERSION% ...
node -e "const fs=require('fs');const v=process.argv[1];const lf=String.fromCharCode(10);const cr=String.fromCharCode(13);const q=String.fromCharCode(34);const eq=String.fromCharCode(61);const writeJson=p=>{const raw=fs.readFileSync(p,'utf8');const nl=raw.indexOf(cr+lf)>=0?cr+lf:lf;const obj=JSON.parse(raw);obj.version=v;fs.writeFileSync(p,JSON.stringify(obj,null,2).split(lf).join(nl)+nl,'utf8')};const writeToml=p=>{const raw=fs.readFileSync(p,'utf8');const nl=raw.indexOf(cr+lf)>=0?cr+lf:lf;const lines=raw.split(nl);let done=false;for(let i=0;i<lines.length;i++){if(!done&&lines[i].startsWith('version'+' '+eq+' ')){lines[i]='version'+' '+eq+' '+q+v+q;done=true}}if(!done)throw new Error('version field not found: '+p);fs.writeFileSync(p,lines.join(nl),'utf8')};writeJson('package.json');writeJson('src-tauri/tauri.conf.json');writeToml('src-tauri/Cargo.toml');" "%VERSION%"
if errorlevel 1 (
  echo [错误] 版本号写入失败。
  goto :fail
)

echo [2/4] 清理旧发布包 ...
powershell -NoProfile -Command "if(Test-Path 'publish'){Get-ChildItem -Path 'publish\*.exe','publish\*.msi','publish\*.zip' -File -ErrorAction SilentlyContinue | Remove-Item -Force}"
if errorlevel 1 (
  echo [错误] 清理 publish 目录失败。
  goto :fail
)

echo [3/4] 构建 Release 安装包 ...
call npm run tauri:build
if errorlevel 1 (
  echo [错误] Tauri Release 构建失败。
  goto :fail
)

echo [4/4] 复制发布包到 publish ...
powershell -NoProfile -Command "New-Item -ItemType Directory -Force -Path 'publish' | Out-Null; Get-ChildItem -Path 'src-tauri\target\release\bundle' -Recurse -File -Include *.exe,*.msi,*.zip -ErrorAction SilentlyContinue | Copy-Item -Destination 'publish' -Force; Get-ChildItem -Path 'publish' -File | Select-Object Name,Length,LastWriteTime | Format-Table -AutoSize"
if errorlevel 1 (
  echo [错误] 复制发布包失败。
  goto :fail
)

echo.
echo 发布完成。
echo 输出目录：%CD%\publish
echo 版本号：%VERSION%
echo.
if "%SKIP_PAUSE%"=="1" exit /b 0
pause
exit /b 0

:fail
echo.
echo 发布失败，请检查上方错误信息。
echo.
if "%SKIP_PAUSE%"=="1" exit /b 1
pause
exit /b 1
