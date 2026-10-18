param(
  [string]$Version = "latest",
  [string]$Prefix = "$HOME\.local",
  [string]$BaseUrl = "",
  [switch]$NoModifyPath,
  [switch]$NoService,
  [switch]$Plugin,
  [switch]$PluginOnly,
  [switch]$Open
)
$ErrorActionPreference = "Stop"

$installRoot = Join-Path $Prefix "lib\wombat"
$binDir = Join-Path $Prefix "bin"
$launcher = Join-Path $binDir "wombat.cmd"
if ($PluginOnly) {
  $NoService = $true
  if ($Version -ne "latest" -or $BaseUrl) { throw "-PluginOnly uses the current installed version; omit -Version and -BaseUrl" }
  $pointer = Join-Path $installRoot "current.txt"
  if (-not (Test-Path (Join-Path $installRoot ".managed-by-wombat") -PathType Leaf) -or -not (Test-Path $pointer -PathType Leaf) -or (Get-Item $installRoot).LinkType -or (Get-Item $pointer).LinkType -or (Get-Item $pointer).Length -gt 128) { throw "A managed Wombat installation is required for -PluginOnly" }
  $releaseId = (Get-Content $pointer -Raw).TrimEnd("`r", "`n")
  if ($releaseId -notmatch '^[0-9A-Za-z][0-9A-Za-z._-]{0,127}$') { throw "Invalid Wombat installation pointer" }
  $destination = Join-Path $installRoot "versions\$releaseId"
  if ((Get-Item $destination).LinkType -or -not (Test-Path (Join-Path $destination "runtime\node.exe") -PathType Leaf) -or -not (Test-Path (Join-Path $destination "lib\wombat.js") -PathType Leaf)) { throw "The current Wombat runtime is incomplete; reinstall it before retrying the plugin" }
  $release = Get-Content (Join-Path $destination "release.json") -Raw | ConvertFrom-Json
  if ($release.format -ne 1 -or $release.version -notmatch '^[0-9A-Za-z][0-9A-Za-z._-]{0,63}$' -or $release.source -notmatch '^[0-9a-f]{40}$' -or $release.sourceSha256 -notmatch '^[0-9a-f]{64}$' -or "$($release.version)-$($release.source.Substring(0,12))-$($release.sourceSha256.Substring(0,12))" -ne $releaseId) { throw "Invalid Wombat release identity" }
  $Plugin = $true
  Write-Host "Reusing the current Wombat runtime for plugin installation."
} else {
if ([Runtime.InteropServices.RuntimeInformation]::OSArchitecture -ne [Runtime.InteropServices.Architecture]::X64) { throw "This release supports Windows x64" }
$tar = Join-Path $env:SystemRoot "System32\tar.exe"
if (-not (Test-Path -LiteralPath $tar -PathType Leaf)) { throw "Windows tar.exe is required" }
$repo = "YannByte/wombat"
$target = "win32-x64"
$archive = "wombat-$target.tar.gz"
if ($Version -eq "latest") { $base = "https://github.com/$repo/releases/latest/download" }
else {
  $tag = if ($Version.StartsWith("v")) { $Version } else { "v$Version" }
  $base = "https://github.com/$repo/releases/download/$tag"
}
if ($BaseUrl) { $base = $BaseUrl.TrimEnd('/') }

$temp = Join-Path ([IO.Path]::GetTempPath()) ("wombat-install-" + [Guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Path $temp | Out-Null
try {
  $archivePath = Join-Path $temp $archive
  $checksums = Join-Path $temp "SHA256SUMS"
  Invoke-WebRequest "$base/$archive" -OutFile $archivePath
  Invoke-WebRequest "$base/SHA256SUMS" -OutFile $checksums
  $line = Get-Content $checksums | Where-Object { $_ -match "^[0-9a-f]{64}\s+$([regex]::Escape($archive))$" } | Select-Object -First 1
  if (-not $line) { throw "Checksum not found for $archive" }
  $expected = ($line -split '\s+')[0]
  $actual = (Get-FileHash -Algorithm SHA256 $archivePath).Hash.ToLowerInvariant()
  if ($actual -ne $expected.ToLowerInvariant()) { throw "Checksum mismatch for $archive" }
  $names = & $tar -tzf $archivePath
  if ($LASTEXITCODE -ne 0 -or -not $names -or ($names | Where-Object { $_ -notmatch '^wombat/?' -or $_ -match '(^|/)\.\.(/|$)' -or $_.StartsWith('/') })) { throw "Unsafe path in $archive" }
  $listing = & $tar -tvzf $archivePath
  if ($LASTEXITCODE -ne 0 -or ($listing | Where-Object { $_ -and $_[0] -notin @('-', 'd') })) { throw "Links or special files are not allowed in $archive" }
  & $tar -xzf $archivePath -C $temp
  if ($LASTEXITCODE -ne 0) { throw "Could not extract $archive" }

  $payload = Join-Path $temp "wombat"
  $runtime = Join-Path $payload "runtime\node.exe"
  $releaseFile = Join-Path $payload "release.json"
  if (-not (Test-Path $runtime) -or -not (Test-Path (Join-Path $payload "lib\wombat.js")) -or -not (Test-Path $releaseFile)) { throw "Invalid Wombat archive" }
  $release = Get-Content $releaseFile -Raw | ConvertFrom-Json
  if ($release.format -ne 1 -or $release.target -ne $target -or $release.version -notmatch '^[0-9A-Za-z][0-9A-Za-z._-]{0,63}$' -or $release.source -notmatch '^[0-9a-f]{40}$' -or $release.sourceSha256 -notmatch '^[0-9a-f]{64}$') { throw "Invalid Wombat release identity" }
  $releaseId = "$($release.version)-$($release.source.Substring(0,12))-$($release.sourceSha256.Substring(0,12))"

  $installRoot = Join-Path $Prefix "lib\wombat"
  $versions = Join-Path $installRoot "versions"
  $binDir = Join-Path $Prefix "bin"
  $launcher = Join-Path $binDir "wombat.cmd"
  $marker = Join-Path $installRoot ".managed-by-wombat"
  New-Item -ItemType Directory -Force -Path (Join-Path $Prefix "lib"), $binDir | Out-Null
  if ((Test-Path $installRoot) -and -not (Test-Path $marker)) { throw "Refusing to replace an unmanaged directory: $installRoot" }
  if ((Test-Path $launcher) -and -not ((Get-Content $launcher -Raw) -match 'managed GitHub installation')) { throw "Refusing to replace an unmanaged command: $launcher" }
  New-Item -ItemType Directory -Force -Path $versions | Out-Null
  $destination = Join-Path $versions $releaseId
  if (-not (Test-Path $destination)) { Move-Item $payload $destination }
  Set-Content -Path $marker -Value "managed GitHub installation"
  $nextPointer = Join-Path $installRoot ".current-$PID"
  Set-Content -Path $nextPointer -Value $releaseId
  Move-Item -Force $nextPointer (Join-Path $installRoot "current.txt")
  $launcherText = @'
@echo off
rem Wombat managed GitHub installation
set "WOMBAT_INSTALL_ROOT=%~dp0..\lib\wombat"
set "WOMBAT_ENDPOINT_FILE=%~dp0..\lib\wombat\endpoint.json"
set /p WOMBAT_RELEASE=<"%~dp0..\lib\wombat\current.txt"
"%~dp0..\lib\wombat\versions\%WOMBAT_RELEASE%\runtime\node.exe" "%~dp0..\lib\wombat\versions\%WOMBAT_RELEASE%\lib\wombat.js" %*
'@
  Set-Content -Path $launcher -Value $launcherText
  $installedVersion = (& $launcher --version --json | ConvertFrom-Json).version
  Write-Host "Installed Wombat $installedVersion for $target"
  if (-not $NoModifyPath) {
    $normalizedBin = [IO.Path]::GetFullPath($binDir).TrimEnd('\')
    $userPath = [Environment]::GetEnvironmentVariable("Path", "User")
    $hasUserPath = ($userPath -split ';' | Where-Object { $_ } | Where-Object {
      try { [IO.Path]::GetFullPath([Environment]::ExpandEnvironmentVariables($_)).TrimEnd('\') -eq $normalizedBin } catch { $false }
    }).Count -gt 0
    if (-not $hasUserPath) {
      $nextUserPath = if ($userPath) { "$userPath;$binDir" } else { $binDir }
      [Environment]::SetEnvironmentVariable("Path", $nextUserPath, "User")
    }
    $hasProcessPath = ($env:PATH -split ';' | Where-Object { $_ } | Where-Object {
      try { [IO.Path]::GetFullPath([Environment]::ExpandEnvironmentVariables($_)).TrimEnd('\') -eq $normalizedBin } catch { $false }
    }).Count -gt 0
    if (-not $hasProcessPath) { $env:PATH = "$binDir;$env:PATH" }
    $resolvedLauncher = Get-Command wombat.cmd -ErrorAction SilentlyContinue
    if (-not $resolvedLauncher -or [IO.Path]::GetFullPath($resolvedLauncher.Source) -ne [IO.Path]::GetFullPath($launcher)) { throw "Could not activate Wombat on PATH" }
    Write-Host "Added $binDir to the user PATH."
  }
}
finally {
  if (Test-Path $temp) { Remove-Item -Recurse -Force $temp }
}
}
if (-not $NoService) {
  Write-Host "Starting background indexing for recent data..."
  $capabilities = & $launcher api --json | ConvertFrom-Json
  if ($capabilities.methods -contains "service") {
    & $launcher service enable --json
    if ($LASTEXITCODE -ne 0) { throw "Wombat is installed; background startup could not be confirmed. Retry wombat service enable --json." }
  } else { Write-Host "This runtime does not provide background service controls. Installed files are retained." }
}
if ($Plugin) {
  $pluginBootstrap = @'
// BEGIN generated Wombat plugin bootstrap
/* Bundled dependency notices
cross-spawn@7.0.6
The MIT License (MIT)

Copyright (c) 2018 Made With MOXY Lda <hello@moxy.studio>

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in
all copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
THE SOFTWARE.

isexe@2.0.0
The ISC License

Copyright (c) Isaac Z. Schlueter and Contributors

Permission to use, copy, modify, and/or distribute this software for any
purpose with or without fee is hereby granted, provided that the above
copyright notice and this permission notice appear in all copies.

THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES
WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR
ANY SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN
ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF OR
IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.

path-key@3.1.1
MIT License

Copyright (c) Sindre Sorhus <sindresorhus@gmail.com> (sindresorhus.com)

Permission is hereby granted, free of charge, to any person obtaining a copy of this software and associated documentation files (the "Software"), to deal in the Software without restriction, including without limitation the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

shebang-command@2.0.0
MIT License

Copyright (c) Kevin Mårtensson <kevinmartensson@gmail.com> (github.com/kevva)

Permission is hereby granted, free of charge, to any person obtaining a copy of this software and associated documentation files (the "Software"), to deal in the Software without restriction, including without limitation the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

shebang-regex@3.0.0
MIT License

Copyright (c) Sindre Sorhus <sindresorhus@gmail.com> (sindresorhus.com)

Permission is hereby granted, free of charge, to any person obtaining a copy of this software and associated documentation files (the "Software"), to deal in the Software without restriction, including without limitation the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

which@2.0.2
The ISC License

Copyright (c) Isaac Z. Schlueter and Contributors

Permission to use, copy, modify, and/or distribute this software for any
purpose with or without fee is hereby granted, provided that the above
copyright notice and this permission notice appear in all copies.

THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES
WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR
ANY SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN
ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF OR
IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.

*/
"use strict";var tt=Object.create;var R=Object.defineProperty;var nt=Object.getOwnPropertyDescriptor;var rt=Object.getOwnPropertyNames;var ot=Object.getPrototypeOf,it=Object.prototype.hasOwnProperty;var S=(e,t)=>()=>(t||e((t={exports:{}}).exports,t),t.exports),st=(e,t)=>{for(var n in t)R(e,n,{get:t[n],enumerable:!0})},ee=(e,t,n,r)=>{if(t&&typeof t=="object"||typeof t=="function")for(let o of rt(t))!it.call(e,o)&&o!==n&&R(e,o,{get:()=>t[o],enumerable:!(r=nt(t,o))||r.enumerable});return e};var G=(e,t,n)=>(n=e!=null?tt(ot(e)):{},ee(t||!e||!e.__esModule?R(n,"default",{value:e,enumerable:!0}):n,e)),at=e=>ee(R({},"__esModule",{value:!0}),e);var ie=S((Ht,oe)=>{oe.exports=re;re.sync=ct;var te=require("fs");function lt(e,t){var n=t.pathExt!==void 0?t.pathExt:process.env.PATHEXT;if(!n||(n=n.split(";"),n.indexOf("")!==-1))return!0;for(var r=0;r<n.length;r++){var o=n[r].toLowerCase();if(o&&e.substr(-o.length).toLowerCase()===o)return!0}return!1}function ne(e,t,n){return!e.isSymbolicLink()&&!e.isFile()?!1:lt(t,n)}function re(e,t,n){te.stat(e,function(r,o){n(r,r?!1:ne(o,e,t))})}function ct(e,t){return ne(te.statSync(e),e,t)}});var ue=S((qt,ce)=>{ce.exports=ae;ae.sync=ut;var se=require("fs");function ae(e,t,n){se.stat(e,function(r,o){n(r,r?!1:le(o,t))})}function ut(e,t){return le(se.statSync(e),t)}function le(e,t){return e.isFile()&&dt(e,t)}function dt(e,t){var n=e.mode,r=e.uid,o=e.gid,s=t.uid!==void 0?t.uid:process.getuid&&process.getuid(),a=t.gid!==void 0?t.gid:process.getgid&&process.getgid(),c=parseInt("100",8),w=parseInt("010",8),l=parseInt("001",8),i=c|w,u=n&l||n&w&&o===a||n&c&&r===s||n&i&&s===0;return u}});var pe=S((Vt,de)=>{var Wt=require("fs"),F;process.platform==="win32"||global.TESTING_WINDOWS?F=ie():F=ue();de.exports=H;H.sync=pt;function H(e,t,n){if(typeof t=="function"&&(n=t,t={}),!n){if(typeof Promise!="function")throw new TypeError("callback not provided");return new Promise(function(r,o){H(e,t||{},function(s,a){s?o(s):r(a)})})}F(e,t||{},function(r,o){r&&(r.code==="EACCES"||t&&t.ignoreErrors)&&(r=null,o=!1),n(r,o)})}function pt(e,t){try{return F.sync(e,t||{})}catch(n){if(t&&t.ignoreErrors||n.code==="EACCES")return!1;throw n}}});var be=S((zt,ye)=>{var U=process.platform==="win32"||process.env.OSTYPE==="cygwin"||process.env.OSTYPE==="msys",me=require("path"),mt=U?";":":",fe=pe(),ge=e=>Object.assign(new Error(`not found: ${e}`),{code:"ENOENT"}),he=(e,t)=>{let n=t.colon||mt,r=e.match(/\//)||U&&e.match(/\\/)?[""]:[...U?[process.cwd()]:[],...(t.path||process.env.PATH||"").split(n)],o=U?t.pathExt||process.env.PATHEXT||".EXE;.CMD;.BAT;.COM":"",s=U?o.split(n):[""];return U&&e.indexOf(".")!==-1&&s[0]!==""&&s.unshift(""),{pathEnv:r,pathExt:s,pathExtExe:o}},we=(e,t,n)=>{typeof t=="function"&&(n=t,t={}),t||(t={});let{pathEnv:r,pathExt:o,pathExtExe:s}=he(e,t),a=[],c=l=>new Promise((i,u)=>{if(l===r.length)return t.all&&a.length?i(a):u(ge(e));let d=r[l],m=/^".*"$/.test(d)?d.slice(1,-1):d,b=me.join(m,e),y=!m&&/^\.[\\\/]/.test(e)?e.slice(0,2)+b:b;i(w(y,l,0))}),w=(l,i,u)=>new Promise((d,m)=>{if(u===o.length)return d(c(i+1));let b=o[u];fe(l+b,{pathExt:s},(y,v)=>{if(!y&&v)if(t.all)a.push(l+b);else return d(l+b);return d(w(l,i,u+1))})});return n?c(0).then(l=>n(null,l),n):c(0)},ft=(e,t)=>{t=t||{};let{pathEnv:n,pathExt:r,pathExtExe:o}=he(e,t),s=[];for(let a=0;a<n.length;a++){let c=n[a],w=/^".*"$/.test(c)?c.slice(1,-1):c,l=me.join(w,e),i=!w&&/^\.[\\\/]/.test(e)?e.slice(0,2)+l:l;for(let u=0;u<r.length;u++){let d=i+r[u];try{if(fe.sync(d,{pathExt:o}))if(t.all)s.push(d);else return d}catch{}}}if(t.all&&s.length)return s;if(t.nothrow)return null;throw ge(e)};ye.exports=we;we.sync=ft});var Se=S((Jt,q)=>{"use strict";var Ee=(e={})=>{let t=e.env||process.env;return(e.platform||process.platform)!=="win32"?"PATH":Object.keys(t).reverse().find(r=>r.toUpperCase()==="PATH")||"Path"};q.exports=Ee;q.exports.default=Ee});var ke=S((Kt,Ce)=>{"use strict";var xe=require("path"),gt=be(),ht=Se();function ve(e,t){let n=e.options.env||process.env,r=process.cwd(),o=e.options.cwd!=null,s=o&&process.chdir!==void 0&&!process.chdir.disabled;if(s)try{process.chdir(e.options.cwd)}catch{}let a;try{a=gt.sync(e.command,{path:n[ht({env:n})],pathExt:t?xe.delimiter:void 0})}catch{}finally{s&&process.chdir(r)}return a&&(a=xe.resolve(o?e.options.cwd:"",a)),a}function wt(e){return ve(e)||ve(e,!0)}Ce.exports=wt});var Ie=S((Xt,V)=>{"use strict";var W=/([()\][%!^"`<>&|;, *?])/g;function yt(e){return e=e.replace(W,"^$1"),e}function bt(e,t){return e=`${e}`,e=e.replace(/(?=(\\+?)?)\1"/g,'$1$1\\"'),e=e.replace(/(?=(\\+?)?)\1$/,"$1$1"),e=`"${e}"`,e=e.replace(W,"^$1"),t&&(e=e.replace(W,"^$1")),e}V.exports.command=yt;V.exports.argument=bt});var Pe=S((Yt,Ae)=>{"use strict";Ae.exports=/^#!(.*)/});var Ne=S((Zt,Oe)=>{"use strict";var Et=Pe();Oe.exports=(e="")=>{let t=e.match(Et);if(!t)return null;let[n,r]=t[0].replace(/#! ?/,"").split(" "),o=n.split("/").pop();return o==="env"?r:r?`${o} ${r}`:o}});var Le=S((Qt,Te)=>{"use strict";var z=require("fs"),St=Ne();function xt(e){let n=Buffer.alloc(150),r;try{r=z.openSync(e,"r"),z.readSync(r,n,0,150,0),z.closeSync(r)}catch{}return St(n.toString())}Te.exports=xt});var $e=S((en,Ue)=>{"use strict";var vt=require("path"),_e=ke(),je=Ie(),Ct=Le(),kt=process.platform==="win32",It=/\.(?:com|exe)$/i,At=/node_modules[\\/].bin[\\/][^\\/]+\.cmd$/i;function Pt(e){e.file=_e(e);let t=e.file&&Ct(e.file);return t?(e.args.unshift(e.file),e.command=t,_e(e)):e.file}function Ot(e){if(!kt)return e;let t=Pt(e),n=!It.test(t);if(e.options.forceShell||n){let r=At.test(t);e.command=vt.normalize(e.command),e.command=je.command(e.command),e.args=e.args.map(s=>je.argument(s,r));let o=[e.command].concat(e.args).join(" ");e.args=["/d","/s","/c",`"${o}"`],e.command=process.env.comspec||"cmd.exe",e.options.windowsVerbatimArguments=!0}return e}function Nt(e,t,n){t&&!Array.isArray(t)&&(n=t,t=null),t=t?t.slice(0):[],n=Object.assign({},n);let r={command:e,args:t,options:n,file:void 0,original:{command:e,args:t}};return n.shell?r:Ot(r)}Ue.exports=Nt});var Re=S((tn,Me)=>{"use strict";var J=process.platform==="win32";function K(e,t){return Object.assign(new Error(`${t} ${e.command} ENOENT`),{code:"ENOENT",errno:"ENOENT",syscall:`${t} ${e.command}`,path:e.command,spawnargs:e.args})}function Tt(e,t){if(!J)return;let n=e.emit;e.emit=function(r,o){if(r==="exit"){let s=De(o,t);if(s)return n.call(e,"error",s)}return n.apply(e,arguments)}}function De(e,t){return J&&e===1&&!t.file?K(t.original,"spawn"):null}function Lt(e,t){return J&&e===1&&!t.file?K(t.original,"spawnSync"):null}Me.exports={hookChildProcess:Tt,verifyENOENT:De,verifyENOENTSync:Lt,notFoundError:K}});var Ge=S((nn,$)=>{"use strict";var Fe=require("child_process"),X=$e(),Y=Re();function Be(e,t,n){let r=X(e,t,n),o=Fe.spawn(r.command,r.args,r.options);return Y.hookChildProcess(o,r),o}function _t(e,t,n){let r=X(e,t,n),o=Fe.spawnSync(r.command,r.args,r.options);return o.error=o.error||Y.verifyENOENTSync(o.status,r),o}$.exports=Be;$.exports.spawn=Be;$.exports.sync=_t;$.exports._parse=X;$.exports._enoent=Y});var Bt={};st(Bt,{installBundledPlugin:()=>Ke});module.exports=at(Bt);var E=require("node:fs"),x=G(require("node:path"),1),ze=G(require("node:os"),1);var He=require("node:child_process"),qe=G(Ge(),1),M=require("node:fs"),O=require("node:fs");var Z=require("node:timers/promises");function We(e){let t=/\n\[(stdout|stderr)\] /g,n=[...e.matchAll(t)];return n.map((r,o)=>r[1]==="stdout"?e.slice(r.index+r[0].length,n[o+1]?.index??e.length):"").join("")}async function Dt(e){if(!e.pid)return e.exitCode!==null||e.signalCode!==null;let n=(e.exitCode!==null||e.signalCode!==null)&&[e.stdin,e.stdout,e.stderr].every(r=>!r||r.closed)?Promise.resolve():new Promise(r=>e.once("close",()=>r()));if(process.platform==="win32"){let r=(0,He.spawnSync)("taskkill",["/PID",String(e.pid),"/T","/F"],{encoding:"utf8",timeout:5e3,maxBuffer:65536,windowsHide:!0});(r.error||r.status!==0)&&e.kill("SIGKILL")}else{try{process.kill(-e.pid,"SIGTERM")}catch{e.kill("SIGTERM")}await Promise.race([n,(0,Z.setTimeout)(1e3)]);try{process.kill(-e.pid,"SIGKILL")}catch{e.exitCode===null&&e.signalCode===null&&e.kill("SIGKILL")}}return await Promise.race([n.then(()=>!0),(0,Z.setTimeout)(3e3).then(()=>!1)])}async function Ve(e){let[t,...n]=e.command;if(!t)throw new Error("Command cannot be empty");let r=(0,O.openSync)(e.logFile,"wx",384);(0,O.writeSync)(r,`start=${new Date().toISOString()}
timeoutMs=${e.timeoutMs}
command=${JSON.stringify(e.command)}
`),(0,O.closeSync)(r);let o=(0,O.createWriteStream)(e.logFile,{flags:"a"}),s=(0,M.statSync)(e.logFile).size,a=!1,c=!1,w=!1,l=!1,i,u,d,m,b=new Promise(p=>{m=p}),y=(0,qe.default)(t,n,{cwd:e.cwd,env:e.env,detached:process.platform!=="win32",stdio:["ignore","pipe","pipe"],windowsHide:!0}),v=new Promise(p=>y.once("close",k=>p({exitCode:k}))),I=p=>{p==="timeout"&&(a=!0),p==="output"&&(c=!0),p==="signal"&&(w=!0),d??=Dt(y),d.then(m)},P=()=>I("signal");e.signal?.addEventListener("abort",P,{once:!0}),e.signal?.aborted&&P();let C=setTimeout(()=>I("timeout"),e.timeoutMs),L=(p,k)=>{if(c||u)return;let j=Buffer.from(`
[${p}] `),N=e.maxBytes-s;if(N<=j.length||k.byteLength>N-j.length){let h=Buffer.from(`
[output budget exceeded; process terminated]
`);N>0&&o.write(h.subarray(0,N)),s=e.maxBytes,I("output");return}s+=j.length+k.byteLength,o.write(j),o.write(k)};y.stdout?.on("data",p=>L("stdout",p)),y.stderr?.on("data",p=>L("stderr",p)),y.once("error",p=>{i=p}),o.once("error",p=>{u=p,I("output")});let A,_=!1;try{A=await Promise.race([v,b.then(k=>{_=!k})])}finally{clearTimeout(C),e.signal?.removeEventListener("abort",P),d&&(l=!await d),(_||l)&&(y.stdout?.destroy(),y.stderr?.destroy()),await new Promise(p=>{o.closed?p():(o.once("finish",()=>p()),o.once("close",()=>p()),o.end())})}return{exitCode:A?.exitCode??y.exitCode,timedOut:a,outputLimit:c,interrupted:w,closeTimedOut:l,...u?{logError:u.message}:{},...i?{spawnError:i}:{}}}var g=e=>!!e&&typeof e=="object"&&!Array.isArray(e),T=class extends Error{code;recovery;constructor(t,n,r){super(n),this.code=t,this.recovery=r}},D=e=>typeof e=="string"&&/^[A-Z][A-Z0-9_]{0,63}$/.test(e)?e:void 0,Je=e=>"'"+e.replaceAll("'",process.platform==="win32"?"''":"'\\''")+"'";function Mt(e,t,n){let r=`Run only the read-only check: wombat setup --project ${Je(process.cwd())} --json. Do not reinstall the runtime.`,o=(i,u,d=r)=>new T(i,u,d);if(!g(e)||e.outputVersion!==1||!g(e.discovery)||!Array.isArray(e.discovery.instances)||!Array.isArray(e.runtimeChecks))return o("SETUP_INVALID_RESPONSE","The verification response is invalid.");let s=e.discovery;if(s.status!=="available"){let i=D(s.errorCode)??{missing:"PLUGIN_NOT_DISCOVERED",disabled:"PLUGIN_DISABLED",ambiguous:"PLUGIN_AMBIGUOUS"}[String(s.status)]??"PLUGIN_DISCOVERY_UNAVAILABLE",u={LOCAL_ACCESS_DENIED:"Local permissions prevented Codex plugin discovery. Run the check in a terminal allowed to access Codex.",CODEX_DISCONNECTED:"Codex disconnected during plugin discovery.",CODEX_TIMEOUT:"Codex plugin discovery timed out.",SETUP_TIMEOUT:"Plugin discovery exceeded the setup deadline.",CANCELLED:"Plugin discovery did not complete before cancellation or the setup deadline.",SKILL_PARSE_ERROR:"Codex reported Skill parsing errors in this project.",PLUGIN_NOT_DISCOVERED:"Codex did not discover the installed Wombat Skill in this project.",PLUGIN_DISABLED:"The Wombat Skill is disabled in this project.",PLUGIN_AMBIGUOUS:"More than one Wombat Skill is enabled in this project."},d=["PLUGIN_DISABLED","PLUGIN_AMBIGUOUS","PLUGIN_NOT_DISCOVERED","SKILL_PARSE_ERROR"].includes(i)?"Check this project's plugin settings and Skill errors in Codex first. ":"";return o(i,u[i]??"Codex plugin discovery is unavailable.",d+r)}let a=Array.isArray(s.instances)?s.instances.filter(i=>g(i)&&i.enabled===!0):[],c=a.length===1&&g(a[0])?a[0]:null;if(!c||c.name!==t+":wombat"||typeof c.path!="string"||!x.default.isAbsolute(c.path))return o("PLUGIN_SELECTION_MISMATCH","Codex selected a different or ambiguous Wombat Skill.","Select one intended Wombat plugin in Codex. "+r);let w=e.runtimeChecks.filter(i=>g(i)&&i.path===c.path);if(w.length!==1||!g(w[0])){let i=Array.isArray(e.errorCodes)&&e.errorCodes.some(u=>["SETUP_TIMEOUT","CANCELLED"].includes(String(u)));return o(i?"SETUP_TIMEOUT":"PLUGIN_RUNTIME_CHECK_MISSING",i?"The setup deadline expired before runtime compatibility could be verified.":"No unique runtime check was returned for the discovered Wombat Skill.")}let l=w[0];if(l.status!=="compatible"){let i=["modified","incompatible","unmanaged"].includes(String(l.status))?"Repair only the plugin with --plugin-only (PowerShell: -PluginOnly); keep any custom prefix. Do not reinstall the runtime.":r;return o(D(l.errorCode)??{modified:"SKILL_RESOURCE_CHANGED",incompatible:"SKILL_RUNTIME_MISMATCH",unmanaged:"PLUGIN_UNMANAGED"}[String(l.status)]??"PLUGIN_RUNTIME_UNAVAILABLE",`The discovered Wombat Skill runtime check is ${["modified","incompatible","unmanaged","unavailable"].includes(String(l.status))?l.status:"invalid"}.`,i)}if(l.pluginVersion!==n.version){let i=typeof l.pluginVersion=="string"&&/^[0-9A-Za-z][0-9A-Za-z._-]{0,63}$/.test(l.pluginVersion)?l.pluginVersion:"unknown";return o("PLUGIN_VERSION_MISMATCH",`Expected plugin ${n.version}; Codex discovered ${i}.`,"Refresh only the plugin with --plugin-only (PowerShell: -PluginOnly); keep any custom prefix. Do not reinstall the runtime.")}if(l.skillContentHash!==n.skillContentHash)return o("PLUGIN_CONTENT_MISMATCH","The discovered Skill content differs from the installed runtime bundle.","Repair only the plugin with --plugin-only (PowerShell: -PluginOnly); keep any custom prefix. Do not reinstall the runtime.")}function Rt(e){if(!g(e))return{};let t=new Set(["available","unavailable","missing","disabled","ambiguous","compatible","modified","incompatible","unmanaged"]),n=o=>typeof o=="string"&&o.length<=4096&&!/[\x00-\x1f\x7f]/.test(o)?o:null,r=g(e.discovery)?e.discovery:{};return{discovery:{status:t.has(String(r.status))?r.status:null,errorCode:D(r.errorCode)??null,instances:Array.isArray(r.instances)?r.instances.slice(0,32).filter(g).map(o=>({name:n(o.name),path:n(o.path),enabled:o.enabled===!0})):[]},runtimeVersion:n(e.runtimeVersion),runtimeChecks:Array.isArray(e.runtimeChecks)?e.runtimeChecks.slice(0,32).filter(g).map(o=>({path:n(o.path),status:t.has(String(o.status))?o.status:null,pluginVersion:n(o.pluginVersion),skillContentHash:typeof o.skillContentHash=="string"&&/^[a-f0-9]{64}$/.test(o.skillContentHash)?o.skillContentHash:null,missingCapabilities:Array.isArray(o.missingCapabilities)?o.missingCapabilities.slice(0,32).map(n).filter(Boolean):[],errorCode:D(o.errorCode)??null})):[],errorCodes:Array.isArray(e.errorCodes)?e.errorCodes.slice(0,32).map(D).filter(Boolean):[]}}function Ft(e){let t=x.default.join(e,".agents/plugins/marketplace.json"),n=(0,E.lstatSync)(t);if(!n.isFile()||n.isSymbolicLink()||n.size>65536)throw new Error("Invalid bundled plugin marketplace.");let r=JSON.parse((0,E.readFileSync)(t,"utf8"));if(!g(r)||r.name!=="wombat-local"||!Array.isArray(r.plugins))throw new Error("Invalid bundled plugin marketplace.");let o=r.plugins;if(!["wombat","wombat-collection"].every(s=>o.some(a=>g(a)&&a.name===s&&g(a.source)&&a.source.source==="local"&&a.source.path===(s==="wombat"?"./plugin":"./collection-plugin"))))throw new Error("Invalid bundled plugin marketplace.")}async function Ke(e,t={}){let n=x.default.resolve(e);Ft(n);let r=t.env??process.env,o=t.binary??r.WOMBAT_CODEX_BIN??"codex",s=/\.[cm]?js$/.test(o)?[process.execPath,o]:[o],a=(0,E.mkdtempSync)(x.default.join(ze.default.tmpdir(),"wombat-plugin-install-")),c=new AbortController,w=()=>c.abort(),l=t.signal?AbortSignal.any([t.signal,c.signal]):c.signal,i="list",u=!1,d={};process.once("SIGINT",w),process.once("SIGTERM",w);try{let m=async(h,B,Xe=s,Ye=[0])=>{i=h;let Q=x.default.join(a,i+".log"),f=await Ve({command:[...Xe,...B],cwd:process.cwd(),env:{...r,WOMBAT_CODEX_BIN:o},logFile:Q,timeoutMs:t.timeoutMs??2e4,maxBytes:1024*1024,signal:l});if(f.exitCode===null||!Ye.includes(f.exitCode)||f.timedOut||f.outputLimit||f.interrupted||f.closeTimedOut||f.spawnError||f.logError){let Ze=f.spawnError?"executable unavailable":f.timedOut?"timeout":f.outputLimit?"output limit":f.interrupted?"cancelled":f.closeTimedOut?"cleanup incomplete":f.logError?"log write failed":"exit "+f.exitCode,Qe=f.spawnError?("code"in f.spawnError?D(f.spawnError.code):void 0)??"EXECUTABLE_UNAVAILABLE":f.timedOut?"TIMEOUT":f.outputLimit?"OUTPUT_LIMIT":f.interrupted?"CANCELLED":f.closeTimedOut?"CLEANUP_INCOMPLETE":f.logError?"LOG_WRITE_FAILED":"COMMAND_FAILED",et=i==="verify"?`Run only the read-only check: wombat setup --project ${Je(process.cwd())} --json. Do not reinstall the runtime.`:"Retry only the plugin stage with --plugin-only (PowerShell: -PluginOnly); keep any custom prefix. Do not reinstall the runtime.";throw new T(Qe,`Codex plugin ${i} failed (${Ze}).`,et)}return We((0,E.readFileSync)(Q,"utf8"))},b=JSON.parse(await m("list",["plugin","list","--marketplace","wombat-local","--json"]));if(!g(b)||!Array.isArray(b.installed))throw new Error("Unrecognized Codex plugin list. Wombat is installed.");let y=b.installed.filter(h=>g(h)&&h.enabled===!0&&typeof h.pluginId=="string"&&["wombat@wombat-local","wombat-collection@wombat-local"].includes(h.pluginId));if(y.length>1)throw new Error("Multiple Wombat plugins are enabled. Choose one in Codex, then rerun the installer.");let v=y.some(h=>g(h)&&h.pluginId==="wombat-collection@wombat-local")?"wombat-collection":"wombat";i="requirements";let I=x.default.join(n,v==="wombat"?"plugin":"collection-plugin","wombat-runtime.json"),P=(0,E.lstatSync)(I);if(!P.isFile()||P.isSymbolicLink()||P.size>65536)throw new Error("Invalid bundled runtime requirements.");let C=JSON.parse((0,E.readFileSync)(I,"utf8"));if(!g(C)||C.format!==1||typeof C.version!="string"||!/^[0-9A-Za-z][0-9A-Za-z._-]{0,63}$/.test(C.version)||typeof C.skillContentHash!="string"||!/^[a-f0-9]{64}$/.test(C.skillContentHash))throw new Error("Invalid bundled runtime requirements.");d={plugin:v,expected:{pluginVersion:C.version,skillContentHash:C.skillContentHash}};let L=JSON.parse(await m("catalogs",["plugin","marketplace","list","--json"]));if(!g(L)||!Array.isArray(L.marketplaces))throw new Error("Unrecognized Codex marketplace list. Wombat is installed.");let A=L.marketplaces.find(h=>g(h)&&h.name==="wombat-local"),_;if(A!==void 0){if(!g(A)||!g(A.marketplaceSource)||A.marketplaceSource.sourceType!=="local"||typeof A.marketplaceSource.source!="string")throw new Error("Existing wombat-local marketplace is not local. Choose its source in Codex before retrying.");x.default.resolve(A.marketplaceSource.source)!==n&&(_=A.marketplaceSource.source,await m("unregister",["plugin","marketplace","remove","wombat-local"]))}let p=!1;try{await m("marketplace",["plugin","marketplace","add",n]),p=!0;let h=JSON.parse(await m("install",["plugin","add",v+"@wombat-local","--json"]));if(!g(h)||h.pluginId!==v+"@wombat-local"||typeof h.installedPath!="string"||!x.default.isAbsolute(h.installedPath))throw new Error("Unrecognized Codex plugin installation result. Check the plugin in Codex.")}catch(h){let B=i;if(_&&!l.aborted)try{p&&await m("restore-remove",["plugin","marketplace","remove","wombat-local"]),await m("restore",["plugin","marketplace","add",_])}catch{throw new Error("Plugin installation and marketplace recovery failed. Wombat is installed; restore wombat-local in Codex and rerun the installer.")}throw i=B,h}let k,j=await m("verify",["setup","--project",process.cwd(),"--json"],[process.execPath,x.default.resolve(n,"../wombat.js")],[0,2]);try{k=JSON.parse(j)}catch{throw new T("SETUP_INVALID_RESPONSE","The verification response is not valid JSON.","Run wombat setup --json to check the installed runtime. Do not reinstall the runtime.")}d={...d,...Rt(k)};let N=Mt(k,v,C);if(N)throw N;return u=!0,v}catch(m){for(let P of(0,E.readdirSync)(a))(0,E.rmSync)(x.default.join(a,P),{force:!0});let b=m instanceof T?m:new T("PLUGIN_INSTALL_FAILED",m instanceof SyntaxError?"Codex returned invalid JSON.":m instanceof Error?m.message:"Plugin installation failed.","Check this stage before retrying only the plugin installation with --plugin-only (PowerShell: -PluginOnly)."),y=x.default.join(a,"diagnostic.json"),v={outputVersion:1,stage:i,code:b.code,project:process.cwd(),checkedAt:new Date().toISOString(),...d},I=!1;try{(0,E.writeFileSync)(y,JSON.stringify(v,null,2)+`
`,{mode:384}),I=!0}catch{}throw new Error(`Wombat is installed. Plugin ${i} failed [${b.code}]: ${b.message} ${b.recovery} Installed files and data are retained.${I?" Diagnostic summary: "+y:" Diagnostic summary could not be saved."}`)}finally{process.off("SIGINT",w),process.off("SIGTERM",w),u&&(0,E.rmSync)(a,{recursive:!0,force:!0})}}if(process.argv[1]==="-"||x.default.basename(process.argv[1]??"")==="install-plugin-bootstrap.ts"){let e=process.argv[2];!e||process.argv.length!==3?(console.error("A bundled plugin marketplace path is required."),process.exitCode=2):Ke(e).then(t=>console.log(`Installed and verified ${t} through Codex for ${process.cwd()}. Start a new Codex project conversation and ask $${t}:wombat about your usage. Hook trust is optional for log queries.`),t=>{console.error(t instanceof Error?t.message:"Plugin installation failed. Wombat is installed."),process.exitCode=1})}0&&(module.exports={installBundledPlugin});
// END generated Wombat plugin bootstrap
'@
  $pluginBootstrap | & (Join-Path $destination "runtime\node.exe") --input-type=commonjs - (Join-Path $destination "lib\skill")
  if ($LASTEXITCODE -ne 0) { throw "Codex plugin installation failed. Wombat remains installed; check Codex and rerun with -Plugin." }
}

if ($Open) {
  Write-Host "Starting Wombat..."
  & $launcher web --open
  if ($LASTEXITCODE -ne 0) { throw "Wombat exited with code $LASTEXITCODE" }
}
