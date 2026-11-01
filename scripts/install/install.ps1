param(
  [string]$Version = "latest",
  [string]$Prefix = "$HOME\.local",
  [string]$BaseUrl = "",
  [switch]$NoModifyPath,
  [switch]$Service,
  [switch]$Plugin,
  [switch]$PluginOnly
)
$ErrorActionPreference = "Stop"

$installRoot = Join-Path $Prefix "lib\wombat"
$binDir = Join-Path $Prefix "bin"
$launcher = Join-Path $binDir "wombat.cmd"
if ($PluginOnly) {
  $Service = $false
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
if ($Service) {
  Write-Host "Enabling the optional resident query service..."
  '{"method":"service","params":{"action":"enable"}}' | & $launcher call
  if ($LASTEXITCODE -ne 0) { throw "Wombat is installed; service startup could not be confirmed. Check the reported error and check the service method with wombat api --method service." }
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
"use strict";var tt=Object.create;var U=Object.defineProperty;var nt=Object.getOwnPropertyDescriptor;var rt=Object.getOwnPropertyNames;var ot=Object.getPrototypeOf,it=Object.prototype.hasOwnProperty;var b=(e,t)=>()=>(t||e((t={exports:{}}).exports,t),t.exports),st=(e,t)=>{for(var n in t)U(e,n,{get:t[n],enumerable:!0})},ee=(e,t,n,o)=>{if(t&&typeof t=="object"||typeof t=="function")for(let r of rt(t))!it.call(e,r)&&r!==n&&U(e,r,{get:()=>t[r],enumerable:!(o=nt(t,r))||o.enumerable});return e};var G=(e,t,n)=>(n=e!=null?tt(ot(e)):{},ee(t||!e||!e.__esModule?U(n,"default",{value:e,enumerable:!0}):n,e)),at=e=>ee(U({},"__esModule",{value:!0}),e);var ie=b((Gt,oe)=>{oe.exports=re;re.sync=ct;var te=require("fs");function lt(e,t){var n=t.pathExt!==void 0?t.pathExt:process.env.PATHEXT;if(!n||(n=n.split(";"),n.indexOf("")!==-1))return!0;for(var o=0;o<n.length;o++){var r=n[o].toLowerCase();if(r&&e.substr(-r.length).toLowerCase()===r)return!0}return!1}function ne(e,t,n){return!e.isSymbolicLink()&&!e.isFile()?!1:lt(t,n)}function re(e,t,n){te.stat(e,function(o,r){n(o,o?!1:ne(r,e,t))})}function ct(e,t){return ne(te.statSync(e),e,t)}});var ue=b((qt,ce)=>{ce.exports=ae;ae.sync=ut;var se=require("fs");function ae(e,t,n){se.stat(e,function(o,r){n(o,o?!1:le(r,t))})}function ut(e,t){return le(se.statSync(e),t)}function le(e,t){return e.isFile()&&dt(e,t)}function dt(e,t){var n=e.mode,o=e.uid,r=e.gid,s=t.uid!==void 0?t.uid:process.getuid&&process.getuid(),a=t.gid!==void 0?t.gid:process.getgid&&process.getgid(),c=parseInt("100",8),h=parseInt("010",8),l=parseInt("001",8),i=c|h,u=n&l||n&h&&r===a||n&c&&o===s||n&i&&s===0;return u}});var pe=b((Ht,de)=>{var Vt=require("fs"),M;process.platform==="win32"||global.TESTING_WINDOWS?M=ie():M=ue();de.exports=q;q.sync=pt;function q(e,t,n){if(typeof t=="function"&&(n=t,t={}),!n){if(typeof Promise!="function")throw new TypeError("callback not provided");return new Promise(function(o,r){q(e,t||{},function(s,a){s?r(s):o(a)})})}M(e,t||{},function(o,r){o&&(o.code==="EACCES"||t&&t.ignoreErrors)&&(o=null,r=!1),n(o,r)})}function pt(e,t){try{return M.sync(e,t||{})}catch(n){if(t&&t.ignoreErrors||n.code==="EACCES")return!1;throw n}}});var Ee=b((Wt,we)=>{var $=process.platform==="win32"||process.env.OSTYPE==="cygwin"||process.env.OSTYPE==="msys",fe=require("path"),ft=$?";":":",me=pe(),ge=e=>Object.assign(new Error(`not found: ${e}`),{code:"ENOENT"}),he=(e,t)=>{let n=t.colon||ft,o=e.match(/\//)||$&&e.match(/\\/)?[""]:[...$?[process.cwd()]:[],...(t.path||process.env.PATH||"").split(n)],r=$?t.pathExt||process.env.PATHEXT||".EXE;.CMD;.BAT;.COM":"",s=$?r.split(n):[""];return $&&e.indexOf(".")!==-1&&s[0]!==""&&s.unshift(""),{pathEnv:o,pathExt:s,pathExtExe:r}},ye=(e,t,n)=>{typeof t=="function"&&(n=t,t={}),t||(t={});let{pathEnv:o,pathExt:r,pathExtExe:s}=he(e,t),a=[],c=l=>new Promise((i,u)=>{if(l===o.length)return t.all&&a.length?i(a):u(ge(e));let d=o[l],f=/^".*"$/.test(d)?d.slice(1,-1):d,w=fe.join(f,e),E=!f&&/^\.[\\\/]/.test(e)?e.slice(0,2)+w:w;i(h(E,l,0))}),h=(l,i,u)=>new Promise((d,f)=>{if(u===r.length)return d(c(i+1));let w=r[u];me(l+w,{pathExt:s},(E,C)=>{if(!E&&C)if(t.all)a.push(l+w);else return d(l+w);return d(h(l,i,u+1))})});return n?c(0).then(l=>n(null,l),n):c(0)},mt=(e,t)=>{t=t||{};let{pathEnv:n,pathExt:o,pathExtExe:r}=he(e,t),s=[];for(let a=0;a<n.length;a++){let c=n[a],h=/^".*"$/.test(c)?c.slice(1,-1):c,l=fe.join(h,e),i=!h&&/^\.[\\\/]/.test(e)?e.slice(0,2)+l:l;for(let u=0;u<o.length;u++){let d=i+o[u];try{if(me.sync(d,{pathExt:r}))if(t.all)s.push(d);else return d}catch{}}}if(t.all&&s.length)return s;if(t.nothrow)return null;throw ge(e)};we.exports=ye;ye.sync=mt});var be=b((zt,V)=>{"use strict";var Se=(e={})=>{let t=e.env||process.env;return(e.platform||process.platform)!=="win32"?"PATH":Object.keys(t).reverse().find(o=>o.toUpperCase()==="PATH")||"Path"};V.exports=Se;V.exports.default=Se});var ke=b((Jt,ve)=>{"use strict";var xe=require("path"),gt=Ee(),ht=be();function Ce(e,t){let n=e.options.env||process.env,o=process.cwd(),r=e.options.cwd!=null,s=r&&process.chdir!==void 0&&!process.chdir.disabled;if(s)try{process.chdir(e.options.cwd)}catch{}let a;try{a=gt.sync(e.command,{path:n[ht({env:n})],pathExt:t?xe.delimiter:void 0})}catch{}finally{s&&process.chdir(o)}return a&&(a=xe.resolve(r?e.options.cwd:"",a)),a}function yt(e){return Ce(e)||Ce(e,!0)}ve.exports=yt});var Ie=b((Kt,W)=>{"use strict";var H=/([()\][%!^"`<>&|;, *?])/g;function wt(e){return e=e.replace(H,"^$1"),e}function Et(e,t){return e=`${e}`,e=e.replace(/(?=(\\+?)?)\1"/g,'$1$1\\"'),e=e.replace(/(?=(\\+?)?)\1$/,"$1$1"),e=`"${e}"`,e=e.replace(H,"^$1"),t&&(e=e.replace(H,"^$1")),e}W.exports.command=wt;W.exports.argument=Et});var Pe=b((Xt,Ae)=>{"use strict";Ae.exports=/^#!(.*)/});var Oe=b((Zt,Ne)=>{"use strict";var St=Pe();Ne.exports=(e="")=>{let t=e.match(St);if(!t)return null;let[n,o]=t[0].replace(/#! ?/,"").split(" "),r=n.split("/").pop();return r==="env"?o:o?`${r} ${o}`:r}});var Te=b((Yt,Le)=>{"use strict";var z=require("fs"),bt=Oe();function xt(e){let n=Buffer.alloc(150),o;try{o=z.openSync(e,"r"),z.readSync(o,n,0,150,0),z.closeSync(o)}catch{}return bt(n.toString())}Le.exports=xt});var De=b((Qt,$e)=>{"use strict";var Ct=require("path"),_e=ke(),je=Ie(),vt=Te(),kt=process.platform==="win32",It=/\.(?:com|exe)$/i,At=/node_modules[\\/].bin[\\/][^\\/]+\.cmd$/i;function Pt(e){e.file=_e(e);let t=e.file&&vt(e.file);return t?(e.args.unshift(e.file),e.command=t,_e(e)):e.file}function Nt(e){if(!kt)return e;let t=Pt(e),n=!It.test(t);if(e.options.forceShell||n){let o=At.test(t);e.command=Ct.normalize(e.command),e.command=je.command(e.command),e.args=e.args.map(s=>je.argument(s,o));let r=[e.command].concat(e.args).join(" ");e.args=["/d","/s","/c",`"${r}"`],e.command=process.env.comspec||"cmd.exe",e.options.windowsVerbatimArguments=!0}return e}function Ot(e,t,n){t&&!Array.isArray(t)&&(n=t,t=null),t=t?t.slice(0):[],n=Object.assign({},n);let o={command:e,args:t,options:n,file:void 0,original:{command:e,args:t}};return n.shell?o:Nt(o)}$e.exports=Ot});var Me=b((en,Ue)=>{"use strict";var J=process.platform==="win32";function K(e,t){return Object.assign(new Error(`${t} ${e.command} ENOENT`),{code:"ENOENT",errno:"ENOENT",syscall:`${t} ${e.command}`,path:e.command,spawnargs:e.args})}function Lt(e,t){if(!J)return;let n=e.emit;e.emit=function(o,r){if(o==="exit"){let s=Re(r,t);if(s)return n.call(e,"error",s)}return n.apply(e,arguments)}}function Re(e,t){return J&&e===1&&!t.file?K(t.original,"spawn"):null}function Tt(e,t){return J&&e===1&&!t.file?K(t.original,"spawnSync"):null}Ue.exports={hookChildProcess:Lt,verifyENOENT:Re,verifyENOENTSync:Tt,notFoundError:K}});var Ge=b((tn,D)=>{"use strict";var Fe=require("child_process"),X=De(),Z=Me();function Be(e,t,n){let o=X(e,t,n),r=Fe.spawn(o.command,o.args,o.options);return Z.hookChildProcess(r,o),r}function _t(e,t,n){let o=X(e,t,n),r=Fe.spawnSync(o.command,o.args,o.options);return r.error=r.error||Z.verifyENOENTSync(r.status,o),r}D.exports=Be;D.exports.spawn=Be;D.exports.sync=_t;D.exports._parse=X;D.exports._enoent=Z});var Ft={};st(Ft,{installBundledPlugin:()=>Ke});module.exports=at(Ft);var S=require("node:fs"),x=G(require("node:path"),1),ze=G(require("node:os"),1);var qe=require("node:child_process"),Ve=G(Ge(),1),F=require("node:fs"),N=require("node:fs");var Y=require("node:timers/promises");function He(e){let t=/\n\[(stdout|stderr)\] /g,n=[...e.matchAll(t)];return n.map((o,r)=>o[1]==="stdout"?e.slice(o.index+o[0].length,n[r+1]?.index??e.length):"").join("")}async function Dt(e){if(!e.pid)return e.exitCode!==null||e.signalCode!==null;let n=(e.exitCode!==null||e.signalCode!==null)&&[e.stdin,e.stdout,e.stderr].every(o=>!o||o.closed)?Promise.resolve():new Promise(o=>e.once("close",()=>o()));if(process.platform==="win32"){let o=(0,qe.spawnSync)("taskkill",["/PID",String(e.pid),"/T","/F"],{encoding:"utf8",timeout:5e3,maxBuffer:65536,windowsHide:!0});(o.error||o.status!==0)&&e.kill("SIGKILL")}else{try{process.kill(-e.pid,"SIGTERM")}catch{e.kill("SIGTERM")}await Promise.race([n,(0,Y.setTimeout)(1e3)]);try{process.kill(-e.pid,"SIGKILL")}catch{e.exitCode===null&&e.signalCode===null&&e.kill("SIGKILL")}}return await Promise.race([n.then(()=>!0),(0,Y.setTimeout)(3e3).then(()=>!1)])}async function We(e){let[t,...n]=e.command;if(!t)throw new Error("Command cannot be empty");let o=(0,N.openSync)(e.logFile,"wx",384);(0,N.writeSync)(o,`start=${new Date().toISOString()}
timeoutMs=${e.timeoutMs}
command=${JSON.stringify(e.command)}
`),(0,N.closeSync)(o);let r=(0,N.createWriteStream)(e.logFile,{flags:"a"}),s=(0,F.statSync)(e.logFile).size,a=!1,c=!1,h=!1,l=!1,i,u,d,f,w=new Promise(p=>{f=p}),E=(0,Ve.default)(t,n,{cwd:e.cwd,env:e.env,detached:process.platform!=="win32",stdio:["ignore","pipe","pipe"],windowsHide:!0}),C=new Promise(p=>E.once("close",k=>p({exitCode:k}))),I=p=>{p==="timeout"&&(a=!0),p==="output"&&(c=!0),p==="signal"&&(h=!0),d??=Dt(E),d.then(f)},P=()=>I("signal");e.signal?.addEventListener("abort",P,{once:!0}),e.signal?.aborted&&P();let v=setTimeout(()=>I("timeout"),e.timeoutMs),T=(p,k)=>{if(c||u)return;let j=Buffer.from(`
[${p}] `),O=e.maxBytes-s;if(O<=j.length||k.byteLength>O-j.length){let y=Buffer.from(`
[output budget exceeded; process terminated]
`);O>0&&r.write(y.subarray(0,O)),s=e.maxBytes,I("output");return}s+=j.length+k.byteLength,r.write(j),r.write(k)};E.stdout?.on("data",p=>T("stdout",p)),E.stderr?.on("data",p=>T("stderr",p)),E.once("error",p=>{i=p}),r.once("error",p=>{u=p,I("output")});let A,_=!1;try{A=await Promise.race([C,w.then(k=>{_=!k})])}finally{clearTimeout(v),e.signal?.removeEventListener("abort",P),d&&(l=!await d),(_||l)&&(E.stdout?.destroy(),E.stderr?.destroy()),await new Promise(p=>{r.closed?p():(r.once("finish",()=>p()),r.once("close",()=>p()),r.end())})}return{exitCode:A?.exitCode??E.exitCode,timedOut:a,outputLimit:c,interrupted:h,closeTimedOut:l,...u?{logError:u.message}:{},...i?{spawnError:i}:{}}}var g=e=>!!e&&typeof e=="object"&&!Array.isArray(e),L=class extends Error{code;recovery;constructor(t,n,o){super(n),this.code=t,this.recovery=o}},R=e=>typeof e=="string"&&/^[A-Z][A-Z0-9_]{0,63}$/.test(e)?e:void 0,Je=e=>"'"+e.replaceAll("'",process.platform==="win32"?"''":"'\\''")+"'";function Rt(e,t,n){let o=`Run only the read-only check: wombat system doctor --project ${Je(process.cwd())} --json. Do not reinstall the runtime.`,r=(i,u,d=o)=>new L(i,u,d);if(!g(e)||e.outputVersion!==1||!g(e.discovery)||!Array.isArray(e.discovery.instances)||!Array.isArray(e.runtimeChecks))return r("SETUP_INVALID_RESPONSE","The verification response is invalid.");let s=e.discovery;if(s.status!=="available"){let i=R(s.errorCode)??{missing:"PLUGIN_NOT_DISCOVERED",disabled:"PLUGIN_DISABLED",ambiguous:"PLUGIN_AMBIGUOUS"}[String(s.status)]??"PLUGIN_DISCOVERY_UNAVAILABLE",u={LOCAL_ACCESS_DENIED:"Local permissions prevented Codex plugin discovery. Run the check in a terminal allowed to access Codex.",CODEX_DISCONNECTED:"Codex disconnected during plugin discovery.",CODEX_TIMEOUT:"Codex plugin discovery timed out.",SETUP_TIMEOUT:"Plugin discovery exceeded the setup deadline.",CANCELLED:"Plugin discovery did not complete before cancellation or the setup deadline.",SKILL_PARSE_ERROR:"Codex reported Skill parsing errors in this project.",PLUGIN_NOT_DISCOVERED:"Codex did not discover the installed Wombat Skill in this project.",PLUGIN_DISABLED:"The Wombat Skill is disabled in this project.",PLUGIN_AMBIGUOUS:"More than one Wombat Skill is enabled in this project."},d=["PLUGIN_DISABLED","PLUGIN_AMBIGUOUS","PLUGIN_NOT_DISCOVERED","SKILL_PARSE_ERROR"].includes(i)?"Check this project's plugin settings and Skill errors in Codex first. ":"";return r(i,u[i]??"Codex plugin discovery is unavailable.",d+o)}let a=Array.isArray(s.instances)?s.instances.filter(i=>g(i)&&i.enabled===!0):[],c=a.length===1&&g(a[0])?a[0]:null;if(!c||c.name!==t+":wombat"||typeof c.path!="string"||!x.default.isAbsolute(c.path))return r("PLUGIN_SELECTION_MISMATCH","Codex selected a different or ambiguous Wombat Skill.","Select one intended Wombat plugin in Codex. "+o);let h=e.runtimeChecks.filter(i=>g(i)&&i.path===c.path);if(h.length!==1||!g(h[0])){let i=Array.isArray(e.errorCodes)&&e.errorCodes.some(u=>["SETUP_TIMEOUT","CANCELLED"].includes(String(u)));return r(i?"SETUP_TIMEOUT":"PLUGIN_RUNTIME_CHECK_MISSING",i?"The setup deadline expired before runtime compatibility could be verified.":"No unique runtime check was returned for the discovered Wombat Skill.")}let l=h[0];if(l.status!=="compatible"){let i=["modified","incompatible","unmanaged"].includes(String(l.status))?"Repair only the plugin with --plugin-only (PowerShell: -PluginOnly); keep any custom prefix. Do not reinstall the runtime.":o;return r(R(l.errorCode)??{modified:"SKILL_RESOURCE_CHANGED",incompatible:"SKILL_RUNTIME_MISMATCH",unmanaged:"PLUGIN_UNMANAGED"}[String(l.status)]??"PLUGIN_RUNTIME_UNAVAILABLE",`The discovered Wombat Skill runtime check is ${["modified","incompatible","unmanaged","unavailable"].includes(String(l.status))?l.status:"invalid"}.`,i)}if(l.pluginVersion!==n.version){let i=typeof l.pluginVersion=="string"&&/^[0-9A-Za-z][0-9A-Za-z._-]{0,63}$/.test(l.pluginVersion)?l.pluginVersion:"unknown";return r("PLUGIN_VERSION_MISMATCH",`Expected plugin ${n.version}; Codex discovered ${i}.`,"Refresh only the plugin with --plugin-only (PowerShell: -PluginOnly); keep any custom prefix. Do not reinstall the runtime.")}if(l.skillContentHash!==n.skillContentHash)return r("PLUGIN_CONTENT_MISMATCH","The discovered Skill content differs from the installed runtime bundle.","Repair only the plugin with --plugin-only (PowerShell: -PluginOnly); keep any custom prefix. Do not reinstall the runtime.")}function Ut(e){if(!g(e))return{};let t=new Set(["available","unavailable","missing","disabled","ambiguous","compatible","modified","incompatible","unmanaged"]),n=r=>typeof r=="string"&&r.length<=4096&&!/[\x00-\x1f\x7f]/.test(r)?r:null,o=g(e.discovery)?e.discovery:{};return{discovery:{status:t.has(String(o.status))?o.status:null,errorCode:R(o.errorCode)??null,instances:Array.isArray(o.instances)?o.instances.slice(0,32).filter(g).map(r=>({name:n(r.name),path:n(r.path),enabled:r.enabled===!0})):[]},runtimeVersion:n(e.runtimeVersion),runtimeChecks:Array.isArray(e.runtimeChecks)?e.runtimeChecks.slice(0,32).filter(g).map(r=>({path:n(r.path),status:t.has(String(r.status))?r.status:null,pluginVersion:n(r.pluginVersion),skillContentHash:typeof r.skillContentHash=="string"&&/^[a-f0-9]{64}$/.test(r.skillContentHash)?r.skillContentHash:null,missingCapabilities:Array.isArray(r.missingCapabilities)?r.missingCapabilities.slice(0,32).map(n).filter(Boolean):[],errorCode:R(r.errorCode)??null})):[],errorCodes:Array.isArray(e.errorCodes)?e.errorCodes.slice(0,32).map(R).filter(Boolean):[]}}function Mt(e){let t=x.default.join(e,".agents/plugins/marketplace.json"),n=(0,S.lstatSync)(t);if(!n.isFile()||n.isSymbolicLink()||n.size>65536)throw new Error("Invalid bundled plugin marketplace.");let o=JSON.parse((0,S.readFileSync)(t,"utf8"));if(!g(o)||o.name!=="wombat-local"||!Array.isArray(o.plugins))throw new Error("Invalid bundled plugin marketplace.");let r=o.plugins;if(r.length!==1||!["wombat"].every(s=>r.some(a=>g(a)&&a.name===s&&g(a.source)&&a.source.source==="local"&&a.source.path==="./plugin")))throw new Error("Invalid bundled plugin marketplace.")}async function Ke(e,t={}){let n=x.default.resolve(e);Mt(n);let o=t.env??process.env,r=t.binary??o.WOMBAT_CODEX_BIN??"codex",s=/\.[cm]?js$/.test(r)?[process.execPath,r]:[r],a=(0,S.mkdtempSync)(x.default.join(ze.default.tmpdir(),"wombat-plugin-install-")),c=new AbortController,h=()=>c.abort(),l=t.signal?AbortSignal.any([t.signal,c.signal]):c.signal,i="list",u=!1,d={};process.once("SIGINT",h),process.once("SIGTERM",h);try{let f=async(y,B,Xe=s,Ze=[0])=>{i=y;let Q=x.default.join(a,i+".log"),m=await We({command:[...Xe,...B],cwd:process.cwd(),env:{...o,WOMBAT_CODEX_BIN:r},logFile:Q,timeoutMs:t.timeoutMs??2e4,maxBytes:1024*1024,signal:l});if(m.exitCode===null||!Ze.includes(m.exitCode)||m.timedOut||m.outputLimit||m.interrupted||m.closeTimedOut||m.spawnError||m.logError){let Ye=m.spawnError?"executable unavailable":m.timedOut?"timeout":m.outputLimit?"output limit":m.interrupted?"cancelled":m.closeTimedOut?"cleanup incomplete":m.logError?"log write failed":"exit "+m.exitCode,Qe=m.spawnError?("code"in m.spawnError?R(m.spawnError.code):void 0)??"EXECUTABLE_UNAVAILABLE":m.timedOut?"TIMEOUT":m.outputLimit?"OUTPUT_LIMIT":m.interrupted?"CANCELLED":m.closeTimedOut?"CLEANUP_INCOMPLETE":m.logError?"LOG_WRITE_FAILED":"COMMAND_FAILED",et=i==="verify"?`Run only the read-only check: wombat system doctor --project ${Je(process.cwd())} --json. Do not reinstall the runtime.`:"Retry only the plugin stage with --plugin-only (PowerShell: -PluginOnly); keep any custom prefix. Do not reinstall the runtime.";throw new L(Qe,`Codex plugin ${i} failed (${Ye}).`,et)}return He((0,S.readFileSync)(Q,"utf8"))},w=JSON.parse(await f("list",["plugin","list","--marketplace","wombat-local","--json"]));if(!g(w)||!Array.isArray(w.installed))throw new Error("Unrecognized Codex plugin list. Wombat is installed.");if(w.installed.filter(y=>g(y)&&y.enabled===!0&&typeof y.pluginId=="string"&&y.pluginId==="wombat@wombat-local").length>1)throw new Error("Multiple Wombat plugins are enabled. Choose one in Codex, then rerun the installer.");let C="wombat";i="requirements";let I=x.default.join(n,"plugin","wombat-runtime.json"),P=(0,S.lstatSync)(I);if(!P.isFile()||P.isSymbolicLink()||P.size>65536)throw new Error("Invalid bundled runtime requirements.");let v=JSON.parse((0,S.readFileSync)(I,"utf8"));if(!g(v)||v.format!==1||typeof v.version!="string"||!/^[0-9A-Za-z][0-9A-Za-z._-]{0,63}$/.test(v.version)||typeof v.skillContentHash!="string"||!/^[a-f0-9]{64}$/.test(v.skillContentHash))throw new Error("Invalid bundled runtime requirements.");d={plugin:C,expected:{pluginVersion:v.version,skillContentHash:v.skillContentHash}};let T=JSON.parse(await f("catalogs",["plugin","marketplace","list","--json"]));if(!g(T)||!Array.isArray(T.marketplaces))throw new Error("Unrecognized Codex marketplace list. Wombat is installed.");let A=T.marketplaces.find(y=>g(y)&&y.name==="wombat-local"),_;if(A!==void 0){if(!g(A)||!g(A.marketplaceSource)||A.marketplaceSource.sourceType!=="local"||typeof A.marketplaceSource.source!="string")throw new Error("Existing wombat-local marketplace is not local. Choose its source in Codex before retrying.");x.default.resolve(A.marketplaceSource.source)!==n&&(_=A.marketplaceSource.source,await f("unregister",["plugin","marketplace","remove","wombat-local"]))}let p=!1;try{await f("marketplace",["plugin","marketplace","add",n]),p=!0;let y=JSON.parse(await f("install",["plugin","add",C+"@wombat-local","--json"]));if(!g(y)||y.pluginId!==C+"@wombat-local"||typeof y.installedPath!="string"||!x.default.isAbsolute(y.installedPath))throw new Error("Unrecognized Codex plugin installation result. Check the plugin in Codex.")}catch(y){let B=i;if(_&&!l.aborted)try{p&&await f("restore-remove",["plugin","marketplace","remove","wombat-local"]),await f("restore",["plugin","marketplace","add",_])}catch{throw new Error("Plugin installation and marketplace recovery failed. Wombat is installed; restore wombat-local in Codex and rerun the installer.")}throw i=B,y}let k,j=await f("verify",["system","doctor","--project",process.cwd(),"--json"],[process.execPath,x.default.resolve(n,"../wombat.js")],[0,2]);try{k=JSON.parse(j)}catch{throw new L("SETUP_INVALID_RESPONSE","The verification response is not valid JSON.","Run wombat system doctor --json to check the installed runtime. Do not reinstall the runtime.")}d={...d,...Ut(k)};let O=Rt(k,C,v);if(O)throw O;return u=!0,C}catch(f){for(let P of(0,S.readdirSync)(a))(0,S.rmSync)(x.default.join(a,P),{force:!0});let w=f instanceof L?f:new L("PLUGIN_INSTALL_FAILED",f instanceof SyntaxError?"Codex returned invalid JSON.":f instanceof Error?f.message:"Plugin installation failed.","Check this stage before retrying only the plugin installation with --plugin-only (PowerShell: -PluginOnly)."),E=x.default.join(a,"diagnostic.json"),C={outputVersion:1,stage:i,code:w.code,project:process.cwd(),checkedAt:new Date().toISOString(),...d},I=!1;try{(0,S.writeFileSync)(E,JSON.stringify(C,null,2)+`
`,{mode:384}),I=!0}catch{}throw new Error(`Wombat is installed. Plugin ${i} failed [${w.code}]: ${w.message} ${w.recovery} Installed files and data are retained.${I?" Diagnostic summary: "+E:" Diagnostic summary could not be saved."}`)}finally{process.off("SIGINT",h),process.off("SIGTERM",h),u&&(0,S.rmSync)(a,{recursive:!0,force:!0})}}if(process.argv[1]==="-"||x.default.basename(process.argv[1]??"")==="install-plugin-bootstrap.ts"){let e=process.argv[2];!e||process.argv.length!==3?(console.error("A bundled plugin marketplace path is required."),process.exitCode=2):Ke(e).then(t=>console.log(`Installed and verified ${t} through Codex for ${process.cwd()}. Start a new Codex project conversation and ask $${t}:wombat about your usage. Hook trust is optional for log queries.`),t=>{console.error(t instanceof Error?t.message:"Plugin installation failed. Wombat is installed."),process.exitCode=1})}0&&(module.exports={installBundledPlugin});
// END generated Wombat plugin bootstrap
'@
  $pluginBootstrap | & (Join-Path $destination "runtime\node.exe") --input-type=commonjs - (Join-Path $destination "lib\skill")
  if ($LASTEXITCODE -ne 0) { throw "Codex plugin installation failed. Wombat remains installed; check Codex and rerun with -Plugin." }
}
