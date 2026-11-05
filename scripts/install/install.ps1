param(
  [string]$Version = "latest",
  [string]$Prefix = "$HOME\.local",
  [string]$BaseUrl = "",
  [switch]$NoModifyPath,
  [switch]$Plugin,
  [switch]$PluginOnly
)
$ErrorActionPreference = "Stop"

$installRoot = Join-Path $Prefix "lib\wombat"
$binDir = Join-Path $Prefix "bin"
$launcher = Join-Path $binDir "wombat.cmd"
if ($PluginOnly) {
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
"use strict";var ot=Object.create;var M=Object.defineProperty;var it=Object.getOwnPropertyDescriptor;var st=Object.getOwnPropertyNames;var at=Object.getPrototypeOf,lt=Object.prototype.hasOwnProperty;var x=(e,t)=>()=>(t||e((t={exports:{}}).exports,t),t.exports),ct=(e,t)=>{for(var n in t)M(e,n,{get:t[n],enumerable:!0})},ne=(e,t,n,o)=>{if(t&&typeof t=="object"||typeof t=="function")for(let r of st(t))!lt.call(e,r)&&r!==n&&M(e,r,{get:()=>t[r],enumerable:!(o=it(t,r))||o.enumerable});return e};var W=(e,t,n)=>(n=e!=null?ot(at(e)):{},ne(t||!e||!e.__esModule?M(n,"default",{value:e,enumerable:!0}):n,e)),ut=e=>ne(M({},"__esModule",{value:!0}),e);var le=x((Wt,ae)=>{ae.exports=se;se.sync=pt;var oe=require("fs");function dt(e,t){var n=t.pathExt!==void 0?t.pathExt:process.env.PATHEXT;if(!n||(n=n.split(";"),n.indexOf("")!==-1))return!0;for(var o=0;o<n.length;o++){var r=n[o].toLowerCase();if(r&&e.substr(-r.length).toLowerCase()===r)return!0}return!1}function ie(e,t,n){return!e.isSymbolicLink()&&!e.isFile()?!1:dt(t,n)}function se(e,t,n){oe.stat(e,function(o,r){n(o,o?!1:ie(r,e,t))})}function pt(e,t){return ie(oe.statSync(e),e,t)}});var fe=x((qt,pe)=>{pe.exports=ue;ue.sync=ft;var ce=require("fs");function ue(e,t,n){ce.stat(e,function(o,r){n(o,o?!1:de(r,t))})}function ft(e,t){return de(ce.statSync(e),t)}function de(e,t){return e.isFile()&&mt(e,t)}function mt(e,t){var n=e.mode,o=e.uid,r=e.gid,i=t.uid!==void 0?t.uid:process.getuid&&process.getuid(),l=t.gid!==void 0?t.gid:process.getgid&&process.getgid(),p=parseInt("100",8),w=parseInt("010",8),c=parseInt("001",8),f=p|w,s=n&c||n&w&&r===l||n&p&&o===i||n&f&&i===0;return s}});var he=x((zt,me)=>{var Ht=require("fs"),F;process.platform==="win32"||global.TESTING_WINDOWS?F=le():F=fe();me.exports=q;q.sync=ht;function q(e,t,n){if(typeof t=="function"&&(n=t,t={}),!n){if(typeof Promise!="function")throw new TypeError("callback not provided");return new Promise(function(o,r){q(e,t||{},function(i,l){i?r(i):o(l)})})}F(e,t||{},function(o,r){o&&(o.code==="EACCES"||t&&t.ignoreErrors)&&(o=null,r=!1),n(o,r)})}function ht(e,t){try{return F.sync(e,t||{})}catch(n){if(t&&t.ignoreErrors||n.code==="EACCES")return!1;throw n}}});var xe=x((Kt,be)=>{var D=process.platform==="win32"||process.env.OSTYPE==="cygwin"||process.env.OSTYPE==="msys",ge=require("path"),gt=D?";":":",ye=he(),we=e=>Object.assign(new Error(`not found: ${e}`),{code:"ENOENT"}),Ee=(e,t)=>{let n=t.colon||gt,o=e.match(/\//)||D&&e.match(/\\/)?[""]:[...D?[process.cwd()]:[],...(t.path||process.env.PATH||"").split(n)],r=D?t.pathExt||process.env.PATHEXT||".EXE;.CMD;.BAT;.COM":"",i=D?r.split(n):[""];return D&&e.indexOf(".")!==-1&&i[0]!==""&&i.unshift(""),{pathEnv:o,pathExt:i,pathExtExe:r}},Se=(e,t,n)=>{typeof t=="function"&&(n=t,t={}),t||(t={});let{pathEnv:o,pathExt:r,pathExtExe:i}=Ee(e,t),l=[],p=c=>new Promise((f,s)=>{if(c===o.length)return t.all&&l.length?f(l):s(we(e));let a=o[c],S=/^".*"$/.test(a)?a.slice(1,-1):a,h=ge.join(S,e),g=!S&&/^\.[\\\/]/.test(e)?e.slice(0,2)+h:h;f(w(g,c,0))}),w=(c,f,s)=>new Promise((a,S)=>{if(s===r.length)return a(p(f+1));let h=r[s];ye(c+h,{pathExt:i},(g,R)=>{if(!g&&R)if(t.all)l.push(c+h);else return a(c+h);return a(w(c,f,s+1))})});return n?p(0).then(c=>n(null,c),n):p(0)},yt=(e,t)=>{t=t||{};let{pathEnv:n,pathExt:o,pathExtExe:r}=Ee(e,t),i=[];for(let l=0;l<n.length;l++){let p=n[l],w=/^".*"$/.test(p)?p.slice(1,-1):p,c=ge.join(w,e),f=!w&&/^\.[\\\/]/.test(e)?e.slice(0,2)+c:c;for(let s=0;s<o.length;s++){let a=f+o[s];try{if(ye.sync(a,{pathExt:r}))if(t.all)i.push(a);else return a}catch{}}}if(t.all&&i.length)return i;if(t.nothrow)return null;throw we(e)};be.exports=Se;Se.sync=yt});var ve=x((Jt,H)=>{"use strict";var Ce=(e={})=>{let t=e.env||process.env;return(e.platform||process.platform)!=="win32"?"PATH":Object.keys(t).reverse().find(o=>o.toUpperCase()==="PATH")||"Path"};H.exports=Ce;H.exports.default=Ce});var Pe=x((Xt,Ie)=>{"use strict";var ke=require("path"),wt=xe(),Et=ve();function Ae(e,t){let n=e.options.env||process.env,o=process.cwd(),r=e.options.cwd!=null,i=r&&process.chdir!==void 0&&!process.chdir.disabled;if(i)try{process.chdir(e.options.cwd)}catch{}let l;try{l=wt.sync(e.command,{path:n[Et({env:n})],pathExt:t?ke.delimiter:void 0})}catch{}finally{i&&process.chdir(o)}return l&&(l=ke.resolve(r?e.options.cwd:"",l)),l}function St(e){return Ae(e)||Ae(e,!0)}Ie.exports=St});var Le=x((Zt,K)=>{"use strict";var z=/([()\][%!^"`<>&|;, *?])/g;function bt(e){return e=e.replace(z,"^$1"),e}function xt(e,t){return e=`${e}`,e=e.replace(/(?=(\\+?)?)\1"/g,'$1$1\\"'),e=e.replace(/(?=(\\+?)?)\1$/,"$1$1"),e=`"${e}"`,e=e.replace(z,"^$1"),t&&(e=e.replace(z,"^$1")),e}K.exports.command=bt;K.exports.argument=xt});var Ne=x((Yt,Oe)=>{"use strict";Oe.exports=/^#!(.*)/});var _e=x((Qt,Te)=>{"use strict";var Ct=Ne();Te.exports=(e="")=>{let t=e.match(Ct);if(!t)return null;let[n,o]=t[0].replace(/#! ?/,"").split(" "),r=n.split("/").pop();return r==="env"?o:o?`${r} ${o}`:r}});var je=x((er,Re)=>{"use strict";var J=require("fs"),vt=_e();function kt(e){let n=Buffer.alloc(150),o;try{o=J.openSync(e,"r"),J.readSync(o,n,0,150,0),J.closeSync(o)}catch{}return vt(n.toString())}Re.exports=kt});var Me=x((tr,Ue)=>{"use strict";var At=require("path"),De=Pe(),$e=Le(),It=je(),Pt=process.platform==="win32",Lt=/\.(?:com|exe)$/i,Ot=/node_modules[\\/].bin[\\/][^\\/]+\.cmd$/i;function Nt(e){e.file=De(e);let t=e.file&&It(e.file);return t?(e.args.unshift(e.file),e.command=t,De(e)):e.file}function Tt(e){if(!Pt)return e;let t=Nt(e),n=!Lt.test(t);if(e.options.forceShell||n){let o=Ot.test(t);e.command=At.normalize(e.command),e.command=$e.command(e.command),e.args=e.args.map(i=>$e.argument(i,o));let r=[e.command].concat(e.args).join(" ");e.args=["/d","/s","/c",`"${r}"`],e.command=process.env.comspec||"cmd.exe",e.options.windowsVerbatimArguments=!0}return e}function _t(e,t,n){t&&!Array.isArray(t)&&(n=t,t=null),t=t?t.slice(0):[],n=Object.assign({},n);let o={command:e,args:t,options:n,file:void 0,original:{command:e,args:t}};return n.shell?o:Tt(o)}Ue.exports=_t});var Ge=x((rr,Be)=>{"use strict";var X=process.platform==="win32";function Z(e,t){return Object.assign(new Error(`${t} ${e.command} ENOENT`),{code:"ENOENT",errno:"ENOENT",syscall:`${t} ${e.command}`,path:e.command,spawnargs:e.args})}function Rt(e,t){if(!X)return;let n=e.emit;e.emit=function(o,r){if(o==="exit"){let i=Fe(r,t);if(i)return n.call(e,"error",i)}return n.apply(e,arguments)}}function Fe(e,t){return X&&e===1&&!t.file?Z(t.original,"spawn"):null}function jt(e,t){return X&&e===1&&!t.file?Z(t.original,"spawnSync"):null}Be.exports={hookChildProcess:Rt,verifyENOENT:Fe,verifyENOENTSync:jt,notFoundError:Z}});var qe=x((nr,$)=>{"use strict";var Ve=require("child_process"),Y=Me(),Q=Ge();function We(e,t,n){let o=Y(e,t,n),r=Ve.spawn(o.command,o.args,o.options);return Q.hookChildProcess(r,o),r}function Dt(e,t,n){let o=Y(e,t,n),r=Ve.spawnSync(o.command,o.args,o.options);return r.error=r.error||Q.verifyENOENTSync(r.status,o),r}$.exports=We;$.exports.spawn=We;$.exports.sync=Dt;$.exports._parse=Y;$.exports._enoent=Q});var Gt={};ct(Gt,{installBundledPlugin:()=>Ye});module.exports=ut(Gt);var E=require("node:fs"),C=W(require("node:path"),1),Xe=W(require("node:os"),1);var He=require("node:child_process"),ze=W(qe(),1),B=require("node:fs"),T=require("node:fs");var ee=require("node:timers/promises");function Ke(e){let t=/\n\[(stdout|stderr)\] /g,n=[...e.matchAll(t)];return n.map((o,r)=>o[1]==="stdout"?e.slice(o.index+o[0].length,n[r+1]?.index??e.length):"").join("")}async function Mt(e){if(!e.pid)return e.exitCode!==null||e.signalCode!==null;let n=(e.exitCode!==null||e.signalCode!==null)&&[e.stdin,e.stdout,e.stderr].every(o=>!o||o.closed)?Promise.resolve():new Promise(o=>e.once("close",()=>o()));if(process.platform==="win32"){let o=(0,He.spawnSync)("taskkill",["/PID",String(e.pid),"/T","/F"],{encoding:"utf8",timeout:5e3,maxBuffer:65536,windowsHide:!0});(o.error||o.status!==0)&&e.kill("SIGKILL")}else{try{process.kill(-e.pid,"SIGTERM")}catch{e.kill("SIGTERM")}await Promise.race([n,(0,ee.setTimeout)(1e3)]);try{process.kill(-e.pid,"SIGKILL")}catch{e.exitCode===null&&e.signalCode===null&&e.kill("SIGKILL")}}return await Promise.race([n.then(()=>!0),(0,ee.setTimeout)(3e3).then(()=>!1)])}async function Je(e){let[t,...n]=e.command;if(!t)throw new Error("Command cannot be empty");let o=(0,T.openSync)(e.logFile,"wx",384);(0,T.writeSync)(o,`start=${new Date().toISOString()}
timeoutMs=${e.timeoutMs}
command=${JSON.stringify(e.command)}
`),(0,T.closeSync)(o);let r=(0,T.createWriteStream)(e.logFile,{flags:"a"}),i=(0,B.statSync)(e.logFile).size,l=!1,p=!1,w=!1,c=!1,f,s,a,S,h=new Promise(u=>{S=u}),g=(0,ze.default)(t,n,{cwd:e.cwd,env:e.env,detached:process.platform!=="win32",stdio:["ignore","pipe","pipe"],windowsHide:!0}),R=new Promise(u=>g.once("close",P=>u({exitCode:P}))),v=u=>{u==="timeout"&&(l=!0),u==="output"&&(p=!0),u==="signal"&&(w=!0),a??=Mt(g),a.then(S)},L=()=>v("signal");e.signal?.addEventListener("abort",L,{once:!0}),e.signal?.aborted&&L();let N=setTimeout(()=>v("timeout"),e.timeoutMs),b=(u,P)=>{if(p||s)return;let I=Buffer.from(`
[${u}] `),j=e.maxBytes-i;if(j<=I.length||P.byteLength>j-I.length){let U=Buffer.from(`
[output budget exceeded; process terminated]
`);j>0&&r.write(U.subarray(0,j)),i=e.maxBytes,v("output");return}i+=I.length+P.byteLength,r.write(I),r.write(P)};g.stdout?.on("data",u=>b("stdout",u)),g.stderr?.on("data",u=>b("stderr",u)),g.once("error",u=>{f=u}),r.once("error",u=>{s=u,v("output")});let A,k=!1;try{A=await Promise.race([R,h.then(P=>{k=!P})])}finally{clearTimeout(N),e.signal?.removeEventListener("abort",L),a&&(c=!await a),(k||c)&&(g.stdout?.destroy(),g.stderr?.destroy()),await new Promise(u=>{r.closed?u():(r.once("finish",()=>u()),r.once("close",()=>u()),r.end())})}return{exitCode:A?.exitCode??g.exitCode,timedOut:l,outputLimit:p,interrupted:w,closeTimedOut:c,...s?{logError:s.message}:{},...f?{spawnError:f}:{}}}var d=e=>!!e&&typeof e=="object"&&!Array.isArray(e),_=class extends Error{code;recovery;constructor(t,n,o){super(n),this.code=t,this.recovery=o}},O=e=>typeof e=="string"&&/^[A-Z][A-Z0-9_]{0,63}$/.test(e)?e:void 0,te=e=>"'"+e.replaceAll("'",process.platform==="win32"?"''":"'\\''")+"'";function Ft(e,t,n){let o=`Run only the read-only check: wombat system doctor --project ${te(process.cwd())} --json. Do not reinstall the runtime.`,r=(s,a,S=o)=>new _(s,a,S);if(d(e)&&d(e.error)&&O(e.error.code))return r(O(e.error.code),"The installed CLI reported a verification error ["+O(e.error.code)+"].");if(!d(e)||e.outputVersion!==1||!d(e.discovery)||!Array.isArray(e.discovery.instances)||!Array.isArray(e.runtimeChecks))return r("SETUP_INVALID_RESPONSE","The verification response is invalid.");let i=e.discovery;if(i.status!=="available"&&i.errorCode!=="SKILL_PARSE_ERROR"){let s=O(i.errorCode)??{missing:"PLUGIN_NOT_DISCOVERED",disabled:"PLUGIN_DISABLED",ambiguous:"PLUGIN_AMBIGUOUS"}[String(i.status)]??"PLUGIN_DISCOVERY_UNAVAILABLE",a={LOCAL_ACCESS_DENIED:"Local permissions prevented Codex plugin discovery. Run the check in a terminal allowed to access Codex.",CODEX_DISCONNECTED:"Codex disconnected during plugin discovery.",CODEX_TIMEOUT:"Codex plugin discovery timed out.",SETUP_TIMEOUT:"Plugin discovery exceeded the setup deadline.",CANCELLED:"Plugin discovery did not complete before cancellation or the setup deadline.",SKILL_PARSE_ERROR:"Codex reported Skill parsing errors in this project.",PLUGIN_NOT_DISCOVERED:"Codex did not discover the installed Wombat Skill in this project.",PLUGIN_DISABLED:"The Wombat Skill is disabled in this project.",PLUGIN_AMBIGUOUS:"More than one Wombat Skill is enabled in this project."},S=["PLUGIN_DISABLED","PLUGIN_AMBIGUOUS","PLUGIN_NOT_DISCOVERED","SKILL_PARSE_ERROR"].includes(s)?"Check this project's plugin settings and Skill errors in Codex first. ":"";return r(s,a[s]??"Codex plugin discovery is unavailable.",S+o)}let l=Array.isArray(i.instances)?i.instances.filter(s=>d(s)&&s.enabled===!0):[],p=l.length===1&&d(l[0])?l[0]:null;if(!p||p.name!==t+":wombat"||typeof p.path!="string"||!C.default.isAbsolute(p.path))return r("PLUGIN_SELECTION_MISMATCH","Codex selected a different or ambiguous Wombat Skill.","Select one intended Wombat plugin in Codex. "+o);if(i.errorCode==="SKILL_PARSE_ERROR"&&Array.isArray(i.parseErrorPaths)&&i.parseErrorPaths.includes(p.path))return r("SKILL_PARSE_ERROR","Codex could not parse the selected Wombat Skill.");let w=e.runtimeChecks.filter(s=>d(s)&&s.path===p.path);if(w.length!==1||!d(w[0])){let s=Array.isArray(e.errorCodes)&&e.errorCodes.some(a=>["SETUP_TIMEOUT","CANCELLED"].includes(String(a)));return r(s?"SETUP_TIMEOUT":"PLUGIN_RUNTIME_CHECK_MISSING",s?"The setup deadline expired before runtime compatibility could be verified.":"No unique runtime check was returned for the discovered Wombat Skill.")}let c=w[0];if(c.status!=="compatible"){let s=["modified","incompatible","unmanaged"].includes(String(c.status))?"Repair only the plugin with --plugin-only (PowerShell: -PluginOnly); keep any custom prefix. Do not reinstall the runtime.":o;return r(O(c.errorCode)??{modified:"SKILL_RESOURCE_CHANGED",incompatible:"SKILL_RUNTIME_MISMATCH",unmanaged:"PLUGIN_UNMANAGED"}[String(c.status)]??"PLUGIN_RUNTIME_UNAVAILABLE",`The discovered Wombat Skill runtime check is ${["modified","incompatible","unmanaged","unavailable"].includes(String(c.status))?c.status:"invalid"}.`,s)}if(c.pluginVersion!==n.version){let s=typeof c.pluginVersion=="string"&&/^[0-9A-Za-z][0-9A-Za-z._-]{0,63}$/.test(c.pluginVersion)?c.pluginVersion:"unknown";return r("PLUGIN_VERSION_MISMATCH",`Expected plugin ${n.version}; Codex discovered ${s}.`,"Refresh only the plugin with --plugin-only (PowerShell: -PluginOnly); keep any custom prefix. Do not reinstall the runtime.")}if(c.skillContentHash!==n.skillContentHash)return r("PLUGIN_CONTENT_MISMATCH","The discovered Skill content differs from the installed runtime bundle.","Repair only the plugin with --plugin-only (PowerShell: -PluginOnly); keep any custom prefix. Do not reinstall the runtime.");let f=Ze(e).installationChecks;if(Array.isArray(f)){let s=f.filter(a=>d(a)&&a.status==="fail").map(a=>d(a)?a.id:"").filter(Boolean);if(s.length)return r("INSTALLATION_CHECK_FAILED","The discovered Wombat bundle is compatible; local installation checks failed: "+s.join(", ")+".")}}function Ze(e){if(!d(e))return{};let t=new Set(["available","unavailable","missing","disabled","ambiguous","compatible","modified","incompatible","unmanaged"]),n=r=>typeof r=="string"&&r.length<=4096&&!/[\x00-\x1f\x7f]/.test(r)?r:null,o=d(e.discovery)?e.discovery:{};return{discovery:{status:t.has(String(o.status))?o.status:null,errorCode:O(o.errorCode)??null,instances:Array.isArray(o.instances)?o.instances.slice(0,32).filter(d).map(r=>({name:n(r.name),path:n(r.path),enabled:r.enabled===!0})):[],parseErrorPaths:Array.isArray(o.parseErrorPaths)?o.parseErrorPaths.slice(0,32).map(n).filter(Boolean):[]},runtimeVersion:n(e.runtimeVersion),installationChecks:Array.isArray(e.installationChecks)?e.installationChecks.slice(0,16).filter(d).filter(r=>["installation","path","runtime","core","codex-source"].includes(String(r.id))&&["pass","warning","fail"].includes(String(r.status))).map(r=>({id:r.id,status:r.status})):[],runtimeChecks:Array.isArray(e.runtimeChecks)?e.runtimeChecks.slice(0,32).filter(d).map(r=>({path:n(r.path),status:t.has(String(r.status))?r.status:null,pluginVersion:n(r.pluginVersion),skillContentHash:typeof r.skillContentHash=="string"&&/^[a-f0-9]{64}$/.test(r.skillContentHash)?r.skillContentHash:null,missingCapabilities:Array.isArray(r.missingCapabilities)?r.missingCapabilities.slice(0,32).map(n).filter(Boolean):[],errorCode:O(r.errorCode)??null})):[],errorCodes:Array.isArray(e.errorCodes)?e.errorCodes.slice(0,32).map(O).filter(Boolean):[]}}function Bt(e){let t=C.default.join(e,".agents/plugins/marketplace.json"),n=(0,E.lstatSync)(t);if(!n.isFile()||n.isSymbolicLink()||n.size>65536)throw new Error("Invalid bundled plugin marketplace.");let o=JSON.parse((0,E.readFileSync)(t,"utf8"));if(!d(o)||o.name!=="wombat-local"||!Array.isArray(o.plugins))throw new Error("Invalid bundled plugin marketplace.");let r=o.plugins;if(r.length!==1||!["wombat"].every(i=>r.some(l=>d(l)&&l.name===i&&d(l.source)&&l.source.source==="local"&&l.source.path==="./plugin")))throw new Error("Invalid bundled plugin marketplace.")}async function Ye(e,t={}){let n=C.default.resolve(e);Bt(n);let o=t.env??process.env,r=t.binary??o.WOMBAT_CODEX_BIN??"codex",i=/\.[cm]?js$/.test(r)?[process.execPath,r]:[r],l=(0,E.mkdtempSync)(C.default.join(Xe.default.tmpdir(),"wombat-plugin-install-")),p=new AbortController,w=()=>p.abort(),c=t.signal?AbortSignal.any([t.signal,p.signal]):p.signal,f="list",s=!1,a={},S=null;process.once("SIGINT",w),process.once("SIGTERM",w);try{let h=async(y,V,Qe=i,et=[0])=>{f=y;let re=C.default.join(l,f+".log"),m=await Je({command:[...Qe,...V],cwd:process.cwd(),env:{...o,WOMBAT_CODEX_BIN:r},logFile:re,timeoutMs:t.timeoutMs??2e4,maxBytes:1024*1024,signal:c});if(y==="verify"&&(S=m.exitCode,a={...a,verificationExitCode:m.exitCode}),m.exitCode===null||!et.includes(m.exitCode)||m.timedOut||m.outputLimit||m.interrupted||m.closeTimedOut||m.spawnError||m.logError){let tt=m.spawnError?"executable unavailable":m.timedOut?"timeout":m.outputLimit?"output limit":m.interrupted?"cancelled":m.closeTimedOut?"cleanup incomplete":m.logError?"log write failed":"exit "+m.exitCode,rt=m.spawnError?("code"in m.spawnError?O(m.spawnError.code):void 0)??"EXECUTABLE_UNAVAILABLE":m.timedOut?"TIMEOUT":m.outputLimit?"OUTPUT_LIMIT":m.interrupted?"CANCELLED":m.closeTimedOut?"CLEANUP_INCOMPLETE":m.logError?"LOG_WRITE_FAILED":"COMMAND_FAILED",nt=f==="verify"?`Run only the read-only check: wombat system doctor --project ${te(process.cwd())} --json. Do not reinstall the runtime.`:"Retry only the plugin stage with --plugin-only (PowerShell: -PluginOnly); keep any custom prefix. Do not reinstall the runtime.";throw new _(rt,`Codex plugin ${f} failed (${tt}).`,nt)}return Ke((0,E.readFileSync)(re,"utf8"))},g=JSON.parse(await h("list",["plugin","list","--marketplace","wombat-local","--json"]));if(!d(g)||!Array.isArray(g.installed))throw new Error("Unrecognized Codex plugin list. Wombat is installed.");if(g.installed.filter(y=>d(y)&&y.enabled===!0&&typeof y.pluginId=="string"&&y.pluginId==="wombat@wombat-local").length>1)throw new Error("Multiple Wombat plugins are enabled. Choose one in Codex, then rerun the installer.");let v="wombat";f="requirements";let L=C.default.join(n,"plugin","wombat-runtime.json"),N=(0,E.lstatSync)(L);if(!N.isFile()||N.isSymbolicLink()||N.size>65536)throw new Error("Invalid bundled runtime requirements.");let b=JSON.parse((0,E.readFileSync)(L,"utf8"));if(!d(b)||b.format!==1||typeof b.version!="string"||!/^[0-9A-Za-z][0-9A-Za-z._-]{0,63}$/.test(b.version)||typeof b.skillContentHash!="string"||!/^[a-f0-9]{64}$/.test(b.skillContentHash))throw new Error("Invalid bundled runtime requirements.");a={plugin:v,expected:{pluginVersion:b.version,skillContentHash:b.skillContentHash}};let A=JSON.parse(await h("catalogs",["plugin","marketplace","list","--json"]));if(!d(A)||!Array.isArray(A.marketplaces))throw new Error("Unrecognized Codex marketplace list. Wombat is installed.");let k=A.marketplaces.find(y=>d(y)&&y.name==="wombat-local"),u;if(k!==void 0){if(!d(k)||!d(k.marketplaceSource)||k.marketplaceSource.sourceType!=="local"||typeof k.marketplaceSource.source!="string")throw new Error("Existing wombat-local marketplace is not local. Choose its source in Codex before retrying.");C.default.resolve(k.marketplaceSource.source)!==n&&(u=k.marketplaceSource.source,await h("unregister",["plugin","marketplace","remove","wombat-local"]))}let P=!1;try{await h("marketplace",["plugin","marketplace","add",n]),P=!0;let y=JSON.parse(await h("install",["plugin","add",v+"@wombat-local","--json"]));if(!d(y)||y.pluginId!==v+"@wombat-local"||typeof y.installedPath!="string"||!C.default.isAbsolute(y.installedPath))throw new Error("Unrecognized Codex plugin installation result. Check the plugin in Codex.")}catch(y){let V=f;if(u&&!c.aborted)try{P&&await h("restore-remove",["plugin","marketplace","remove","wombat-local"]),await h("restore",["plugin","marketplace","add",u])}catch{throw new Error("Plugin installation and marketplace recovery failed. Wombat is installed; restore wombat-local in Codex and rerun the installer.")}throw f=V,y}let I,j=await h("verify",["system","doctor","--project",process.cwd(),"--json"],[process.execPath,C.default.resolve(n,"../wombat.js")],[0,1,2]);try{I=JSON.parse(j)}catch{throw new _("SETUP_INVALID_RESPONSE","The verification response is not valid JSON.","Run wombat system doctor --json to check the installed runtime. Do not reinstall the runtime.")}a={...a,...Ze(I)};let U=Ft(I,v,b);if(U)throw U;if(S===1)throw new _("VERIFY_COMMAND_FAILED","Verification exited with status 1; the returned checks do not explain the failure.","Run wombat system doctor --project "+te(process.cwd())+" --json to inspect this state. Do not reinstall the runtime.");s=!0;let G=d(I)&&d(I.discovery)?I.discovery:{};return{name:v,discoveryVerified:G.status==="available",warningPaths:Array.isArray(G.parseErrorPaths)?G.parseErrorPaths.filter(y=>typeof y=="string"&&y.length<=4096&&!/[\x00-\x1f\x7f]/.test(y)).slice(0,32):[]}}catch(h){for(let u of(0,E.readdirSync)(l))(0,E.rmSync)(C.default.join(l,u),{force:!0});let g=h instanceof _?h:new _("PLUGIN_INSTALL_FAILED",h instanceof SyntaxError?"Codex returned invalid JSON.":h instanceof Error?h.message:"Plugin installation failed.","Check this stage before retrying only the plugin installation with --plugin-only (PowerShell: -PluginOnly)."),R=C.default.join(l,"diagnostic.json"),v={outputVersion:1,stage:f,code:g.code,project:process.cwd(),checkedAt:new Date().toISOString(),...a},L=!1;try{(0,E.writeFileSync)(R,JSON.stringify(v,null,2)+`
`,{mode:384}),L=!0}catch{}let N=a.runtimeChecks,b=g.code!=="INSTALLATION_CHECK_FAILED"&&Array.isArray(N)&&N.some(u=>d(u)&&u.status==="compatible")?" Wombat bundle compatibility passed.":"",A=d(a.discovery)?a.discovery:{},k=A.errorCode==="SKILL_PARSE_ERROR"&&Array.isArray(A.parseErrorPaths)&&A.parseErrorPaths.length?" Skill parsing paths: "+A.parseErrorPaths.join(", ")+".":"";throw new Error(`Wombat is installed. Plugin ${f} failed [${g.code}]: ${g.message}${b}${k} ${g.recovery} Installed files and data are retained.${L?" Diagnostic summary: "+R:" Diagnostic summary could not be saved."}`)}finally{process.off("SIGINT",w),process.off("SIGTERM",w),s&&(0,E.rmSync)(l,{recursive:!0,force:!0})}}if(process.argv[1]==="-"||C.default.basename(process.argv[1]??"")==="install-plugin-bootstrap.ts"){let e=process.argv[2];!e||process.argv.length!==3?(console.error("A bundled plugin marketplace path is required."),process.exitCode=2):Ye(e).then(t=>{t.discoveryVerified?console.log(`Installed and verified ${t.name} through Codex for ${process.cwd()}. Start a new Codex project conversation and ask $${t.name}:wombat about your usage. Hook trust is optional for log queries.`):console.log(`Wombat runtime and plugin files are installed; the discovered Wombat bundle passed compatibility checks. Codex project discovery remains incomplete [SKILL_PARSE_ERROR].${t.warningPaths.length?" Check: "+t.warningPaths.join(", "):" Check Skill parsing errors in Codex."} Fix the reported Skill errors, then run wombat system doctor --json. Do not reinstall Wombat.`)},t=>{console.error(t instanceof Error?t.message:"Plugin installation failed. Wombat is installed."),process.exitCode=1})}0&&(module.exports={installBundledPlugin});
// END generated Wombat plugin bootstrap
'@
  $pluginBootstrap | & (Join-Path $destination "runtime\node.exe") --input-type=commonjs - (Join-Path $destination "lib\skill")
  if ($LASTEXITCODE -ne 0) { throw "Codex plugin installation failed. Wombat remains installed; check Codex and rerun with -Plugin." }
}
