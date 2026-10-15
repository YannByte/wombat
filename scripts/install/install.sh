#!/bin/sh
set -eu

repo="YannByte/wombat"
version="latest"
prefix="${WOMBAT_INSTALL_PREFIX:-$HOME/.local}"
base_url=""
modify_path=1
open_app=0
install_plugin_requested=0
plugin_only=0
start_service=1

while [ "$#" -gt 0 ]; do
  case "$1" in
    --version) version=${2:?missing version}; shift 2 ;;
    --prefix) prefix=${2:?missing prefix}; shift 2 ;;
    --base-url) base_url=${2:?missing base URL}; shift 2 ;;
    --no-service) start_service=0; shift ;;
    --no-modify-path) modify_path=0; shift ;;
    --open) open_app=1; shift ;;
    --plugin) install_plugin_requested=1; shift ;;
    --plugin-only) install_plugin_requested=1; plugin_only=1; shift ;;
    -h|--help) echo "Usage: install.sh [--version VERSION|latest] [--prefix PATH] [--base-url URL] [--no-modify-path] [--no-service] [--plugin|--plugin-only] [--open]"; exit 0 ;;
    *) echo "Unknown argument: $1" >&2; exit 2 ;;
  esac
done

install_root="$prefix/lib/wombat"
bin_dir="$prefix/bin"
launcher="$bin_dir/wombat"
if [ "$plugin_only" -eq 1 ]; then
  start_service=0
  [ "$version" = latest ] && [ -z "$base_url" ] || { echo "--plugin-only uses the current installed version; omit --version and --base-url" >&2; exit 2; }
  [ -f "$install_root/.managed-by-wombat" ] && [ ! -L "$install_root" ] && [ ! -L "$install_root/current.txt" ] || { echo "A managed Wombat installation is required for --plugin-only" >&2; exit 1; }
  [ "$(wc -c < "$install_root/current.txt")" -le 128 ] || { echo "Invalid Wombat installation pointer" >&2; exit 1; }
  IFS= read -r release_id < "$install_root/current.txt"
  case "$release_id" in *[!0-9A-Za-z._-]*|''|.|..) echo "Invalid Wombat installation pointer" >&2; exit 1 ;; esac
  destination="$install_root/versions/$release_id"
  [ -x "$destination/runtime/node" ] && [ -f "$destination/lib/wombat.js" ] && [ -f "$destination/release.json" ] && [ ! -L "$destination" ] || { echo "The current Wombat runtime is incomplete; reinstall it before retrying the plugin" >&2; exit 1; }
  "$destination/runtime/node" -e 'const fs=require("fs");const r=JSON.parse(fs.readFileSync(process.argv[1]));if(r.format!==1||typeof r.version!=="string"||!/^[0-9A-Za-z][0-9A-Za-z._-]{0,63}$/.test(r.version)||!/^[0-9a-f]{40}$/.test(r.source)||!/^[0-9a-f]{64}$/.test(r.sourceSha256)||r.version+"-"+r.source.slice(0,12)+"-"+r.sourceSha256.slice(0,12)!==process.argv[2])process.exit(2)' "$destination/release.json" "$release_id"
  echo "Reusing the current Wombat runtime for plugin installation."
else
command -v curl >/dev/null 2>&1 || { echo "curl is required" >&2; exit 1; }
command -v tar >/dev/null 2>&1 || { echo "tar is required" >&2; exit 1; }
download() {
  curl -fL --http1.1 --retry 3 --retry-all-errors --retry-delay 2 --connect-timeout 15 "$1" -o "$2"
}
case "$(uname -s)" in
  Darwin) os=darwin ;;
  Linux) os=linux ;;
  *) echo "This installer supports macOS and Linux; use install.ps1 on Windows" >&2; exit 1 ;;
esac
case "$(uname -m)" in
  arm64|aarch64) arch=arm64 ;;
  x86_64|amd64) arch=x64 ;;
  *) echo "Unsupported architecture: $(uname -m)" >&2; exit 1 ;;
esac
target="$os-$arch"
archive="wombat-$target.tar.gz"
if [ "$version" = latest ]; then
  base="https://github.com/$repo/releases/latest/download"
else
  case "$version" in v*) tag=$version ;; *) tag="v$version" ;; esac
  base="https://github.com/$repo/releases/download/$tag"
fi
[ -z "$base_url" ] || base=${base_url%/}

tmp=$(mktemp -d "${TMPDIR:-/tmp}/wombat-install.XXXXXX")
trap 'rm -rf "$tmp"' EXIT HUP INT TERM
download "$base/$archive" "$tmp/$archive"
download "$base/SHA256SUMS" "$tmp/SHA256SUMS"
expected=$(awk -v file="$archive" '$2 == file {print $1}' "$tmp/SHA256SUMS")
[ -n "$expected" ] || { echo "Checksum not found for $archive" >&2; exit 1; }
if command -v sha256sum >/dev/null 2>&1; then actual=$(sha256sum "$tmp/$archive" | awk '{print $1}')
else actual=$(shasum -a 256 "$tmp/$archive" | awk '{print $1}'); fi
[ "$actual" = "$expected" ] || { echo "Checksum mismatch for $archive" >&2; exit 1; }
tar -tzf "$tmp/$archive" | awk 'BEGIN { ok=1 } $0 !~ /^wombat\/?/ || $0 ~ /(^|\/)\.\.(\/|$)/ || $0 ~ /^\// { ok=0 } END { exit ok ? 0 : 1 }' || { echo "Unsafe path in $archive" >&2; exit 1; }
tar -tvzf "$tmp/$archive" | awk 'substr($1,1,1) != "-" && substr($1,1,1) != "d" { bad=1 } END { exit bad ? 1 : 0 }' || { echo "Links or special files are not allowed in $archive" >&2; exit 1; }
tar -xzf "$tmp/$archive" -C "$tmp"

payload="$tmp/wombat"
runtime="$payload/runtime/node"
[ -x "$runtime" ] && [ -f "$payload/lib/wombat.js" ] && [ -f "$payload/release.json" ] || { echo "Invalid Wombat archive" >&2; exit 1; }
release_id=$("$runtime" -e 'const fs=require("fs");const r=JSON.parse(fs.readFileSync(process.argv[1]));if(r.format!==1||r.target!==process.argv[2]||!/^[0-9A-Za-z][0-9A-Za-z._-]{0,63}$/.test(r.version)||!/^[0-9a-f]{40}$/.test(r.source)||!/^[0-9a-f]{64}$/.test(r.sourceSha256))process.exit(2);process.stdout.write(r.version+"-"+r.source.slice(0,12)+"-"+r.sourceSha256.slice(0,12))' "$payload/release.json" "$target")
[ -n "$release_id" ] || { echo "Invalid Wombat release identity" >&2; exit 1; }

install_root="$prefix/lib/wombat"
versions="$install_root/versions"
bin_dir="$prefix/bin"
launcher="$bin_dir/wombat"
marker="$install_root/.managed-by-wombat"
mkdir -p "$prefix/lib" "$bin_dir"
if [ -e "$install_root" ] && [ ! -f "$marker" ]; then echo "Refusing to replace an unmanaged directory: $install_root" >&2; exit 1; fi
if [ -e "$launcher" ] && ! grep -q 'managed GitHub installation' "$launcher" 2>/dev/null; then echo "Refusing to replace an unmanaged command: $launcher" >&2; exit 1; fi
mkdir -p "$versions"
destination="$versions/$release_id"
if [ ! -e "$destination" ]; then mv "$payload" "$destination"; fi
printf '%s\n' 'managed GitHub installation' > "$marker"
printf '%s\n' "$release_id" > "$install_root/.current-$$"
mv -f "$install_root/.current-$$" "$install_root/current.txt"
cat > "$launcher" <<'EOF'
#!/bin/sh
# Wombat managed GitHub installation
set -eu
case "$0" in */*) WOMBAT_SCRIPT_DIR=${0%/*} ;; *) WOMBAT_SCRIPT_DIR=. ;; esac
WOMBAT_ROOT=$(CDPATH= cd -- "$WOMBAT_SCRIPT_DIR/../lib/wombat" && pwd)
WOMBAT_INSTALL_ROOT="$WOMBAT_ROOT"
WOMBAT_ENDPOINT_FILE="$WOMBAT_ROOT/endpoint.json"
export WOMBAT_INSTALL_ROOT WOMBAT_ENDPOINT_FILE
IFS= read -r WOMBAT_RELEASE < "$WOMBAT_ROOT/current.txt"
case "$WOMBAT_RELEASE" in *[!0-9A-Za-z._-]*|'') echo "Invalid Wombat installation" >&2; exit 1 ;; esac
exec "$WOMBAT_ROOT/versions/$WOMBAT_RELEASE/runtime/node" "$WOMBAT_ROOT/versions/$WOMBAT_RELEASE/lib/wombat.js" "$@"
EOF
chmod 755 "$launcher"

installed_version=$("$launcher" --version --json | "$destination/runtime/node" -e 'let s="";process.stdin.on("data",c=>s+=c).on("end",()=>process.stdout.write(JSON.parse(s).version))')
echo "Installed Wombat $installed_version for $target"
configure_path() {
  case "${PATH:-}" in
    "$bin_dir"|"$bin_dir:"*) path_was_active=1 ;;
    *) path_was_active=0; PATH="$bin_dir${PATH:+:$PATH}"; export PATH ;;
  esac
  if [ "$modify_path" -eq 1 ] && [ "$prefix" = "$HOME/.local" ]; then
    shell_path=${SHELL:-}
    shell_name=${shell_path##*/}
    case "$shell_name" in
      zsh) profile="$HOME/.zshrc"; path_lines='# Wombat PATH
case ":$PATH:" in
  *":$HOME/.local/bin:"*) ;;
  *) export PATH="$HOME/.local/bin:$PATH" ;;
esac' ;;
      bash) profile="$HOME/.bashrc"; path_lines='# Wombat PATH
case ":$PATH:" in
  *":$HOME/.local/bin:"*) ;;
  *) export PATH="$HOME/.local/bin:$PATH" ;;
esac' ;;
      fish) profile="$HOME/.config/fish/config.fish"; path_lines='# Wombat PATH
fish_add_path "$HOME/.local/bin"' ;;
      *) profile="$HOME/.profile"; path_lines='# Wombat PATH
case ":$PATH:" in
  *":$HOME/.local/bin:"*) ;;
  *) export PATH="$HOME/.local/bin:$PATH" ;;
esac' ;;
    esac
    mkdir -p "${profile%/*}"
    if [ ! -f "$profile" ] || ! grep -Fq '# Wombat PATH' "$profile"; then
      printf '\n%s\n' "$path_lines" >> "$profile"
      echo "Added $bin_dir to PATH in $profile."
    fi
  elif [ "$path_was_active" -eq 0 ]; then
    echo "Add $bin_dir to PATH for future commands."
  fi
}
configure_path
fi
start_service() {
  [ "$start_service" -eq 1 ] || return 0
  echo "Starting background indexing for recent data..."
  if "$launcher" api --json | "$destination/runtime/node" -e 'let s="";process.stdin.on("data",c=>s+=c).on("end",()=>{try{process.exit(JSON.parse(s).methods?.includes("service")?0:1)}catch{process.exit(1)}})'; then
    "$launcher" service enable --json
  else
    echo "This runtime does not provide background service controls. Installed files are retained."
  fi
}
start_service
install_plugin() {
  [ "$install_plugin_requested" -eq 1 ] || return 0
  "$destination/runtime/node" --input-type=commonjs - "$destination/lib/skill" <<'WOMBAT_PLUGIN_BOOTSTRAP'
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
"use strict";var Ye=Object.create;var q=Object.defineProperty;var Xe=Object.getOwnPropertyDescriptor;var Qe=Object.getOwnPropertyNames;var Ze=Object.getPrototypeOf,et=Object.prototype.hasOwnProperty;var b=(e,t)=>()=>(t||e((t={exports:{}}).exports,t),t.exports),tt=(e,t)=>{for(var r in t)q(e,r,{get:t[r],enumerable:!0})},X=(e,t,r,n)=>{if(t&&typeof t=="object"||typeof t=="function")for(let o of Qe(t))!et.call(e,o)&&o!==r&&q(e,o,{get:()=>t[o],enumerable:!(n=Xe(t,o))||n.enumerable});return e};var R=(e,t,r)=>(r=e!=null?Ye(Ze(e)):{},X(t||!e||!e.__esModule?q(r,"default",{value:e,enumerable:!0}):r,e)),rt=e=>X(q({},"__esModule",{value:!0}),e);var re=b((Bt,te)=>{te.exports=ee;ee.sync=ot;var Q=require("fs");function nt(e,t){var r=t.pathExt!==void 0?t.pathExt:process.env.PATHEXT;if(!r||(r=r.split(";"),r.indexOf("")!==-1))return!0;for(var n=0;n<r.length;n++){var o=r[n].toLowerCase();if(o&&e.substr(-o.length).toLowerCase()===o)return!0}return!1}function Z(e,t,r){return!e.isSymbolicLink()&&!e.isFile()?!1:nt(t,r)}function ee(e,t,r){Q.stat(e,function(n,o){r(n,n?!1:Z(o,e,t))})}function ot(e,t){return Z(Q.statSync(e),e,t)}});var ae=b((qt,ie)=>{ie.exports=oe;oe.sync=st;var ne=require("fs");function oe(e,t,r){ne.stat(e,function(n,o){r(n,n?!1:se(o,t))})}function st(e,t){return se(ne.statSync(e),t)}function se(e,t){return e.isFile()&&it(e,t)}function it(e,t){var r=e.mode,n=e.uid,o=e.gid,s=t.uid!==void 0?t.uid:process.getuid&&process.getuid(),i=t.gid!==void 0?t.gid:process.getgid&&process.getgid(),p=parseInt("100",8),g=parseInt("010",8),c=parseInt("001",8),u=p|g,d=r&c||r&g&&o===i||r&p&&n===s||r&u&&s===0;return d}});var ce=b((Rt,le)=>{var Mt=require("fs"),M;process.platform==="win32"||global.TESTING_WINDOWS?M=re():M=ae();le.exports=W;W.sync=at;function W(e,t,r){if(typeof t=="function"&&(r=t,t={}),!r){if(typeof Promise!="function")throw new TypeError("callback not provided");return new Promise(function(n,o){W(e,t||{},function(s,i){s?o(s):n(i)})})}M(e,t||{},function(n,o){n&&(n.code==="EACCES"||t&&t.ignoreErrors)&&(n=null,o=!1),r(n,o)})}function at(e,t){try{return M.sync(e,t||{})}catch(r){if(t&&t.ignoreErrors||r.code==="EACCES")return!1;throw r}}});var he=b((Wt,ge)=>{var j=process.platform==="win32"||process.env.OSTYPE==="cygwin"||process.env.OSTYPE==="msys",ue=require("path"),lt=j?";":":",pe=ce(),de=e=>Object.assign(new Error(`not found: ${e}`),{code:"ENOENT"}),fe=(e,t)=>{let r=t.colon||lt,n=e.match(/\//)||j&&e.match(/\\/)?[""]:[...j?[process.cwd()]:[],...(t.path||process.env.PATH||"").split(r)],o=j?t.pathExt||process.env.PATHEXT||".EXE;.CMD;.BAT;.COM":"",s=j?o.split(r):[""];return j&&e.indexOf(".")!==-1&&s[0]!==""&&s.unshift(""),{pathEnv:n,pathExt:s,pathExtExe:o}},me=(e,t,r)=>{typeof t=="function"&&(r=t,t={}),t||(t={});let{pathEnv:n,pathExt:o,pathExtExe:s}=fe(e,t),i=[],p=c=>new Promise((u,d)=>{if(c===n.length)return t.all&&i.length?u(i):d(de(e));let m=n[c],y=/^".*"$/.test(m)?m.slice(1,-1):m,x=ue.join(y,e),h=!y&&/^\.[\\\/]/.test(e)?e.slice(0,2)+x:x;u(g(h,c,0))}),g=(c,u,d)=>new Promise((m,y)=>{if(d===o.length)return m(p(u+1));let x=o[d];pe(c+x,{pathExt:s},(h,k)=>{if(!h&&k)if(t.all)i.push(c+x);else return m(c+x);return m(g(c,u,d+1))})});return r?p(0).then(c=>r(null,c),r):p(0)},ct=(e,t)=>{t=t||{};let{pathEnv:r,pathExt:n,pathExtExe:o}=fe(e,t),s=[];for(let i=0;i<r.length;i++){let p=r[i],g=/^".*"$/.test(p)?p.slice(1,-1):p,c=ue.join(g,e),u=!g&&/^\.[\\\/]/.test(e)?e.slice(0,2)+c:c;for(let d=0;d<n.length;d++){let m=u+n[d];try{if(pe.sync(m,{pathExt:o}))if(t.all)s.push(m);else return m}catch{}}}if(t.all&&s.length)return s;if(t.nothrow)return null;throw de(e)};ge.exports=me;me.sync=ct});var ye=b((Ht,H)=>{"use strict";var we=(e={})=>{let t=e.env||process.env;return(e.platform||process.platform)!=="win32"?"PATH":Object.keys(t).reverse().find(n=>n.toUpperCase()==="PATH")||"Path"};H.exports=we;H.exports.default=we});var Se=b((zt,Ee)=>{"use strict";var be=require("path"),ut=he(),pt=ye();function xe(e,t){let r=e.options.env||process.env,n=process.cwd(),o=e.options.cwd!=null,s=o&&process.chdir!==void 0&&!process.chdir.disabled;if(s)try{process.chdir(e.options.cwd)}catch{}let i;try{i=ut.sync(e.command,{path:r[pt({env:r})],pathExt:t?be.delimiter:void 0})}catch{}finally{s&&process.chdir(n)}return i&&(i=be.resolve(o?e.options.cwd:"",i)),i}function dt(e){return xe(e)||xe(e,!0)}Ee.exports=dt});var ve=b((Jt,J)=>{"use strict";var z=/([()\][%!^"`<>&|;, *?])/g;function ft(e){return e=e.replace(z,"^$1"),e}function mt(e,t){return e=`${e}`,e=e.replace(/(?=(\\+?)?)\1"/g,'$1$1\\"'),e=e.replace(/(?=(\\+?)?)\1$/,"$1$1"),e=`"${e}"`,e=e.replace(z,"^$1"),t&&(e=e.replace(z,"^$1")),e}J.exports.command=ft;J.exports.argument=mt});var Ce=b((Dt,ke)=>{"use strict";ke.exports=/^#!(.*)/});var Oe=b((_t,Pe)=>{"use strict";var gt=Ce();Pe.exports=(e="")=>{let t=e.match(gt);if(!t)return null;let[r,n]=t[0].replace(/#! ?/,"").split(" "),o=r.split("/").pop();return o==="env"?n:n?`${o} ${n}`:o}});var Ie=b((Gt,Ae)=>{"use strict";var D=require("fs"),ht=Oe();function wt(e){let r=Buffer.alloc(150),n;try{n=D.openSync(e,"r"),D.readSync(n,r,0,150,0),D.closeSync(n)}catch{}return ht(r.toString())}Ae.exports=wt});var $e=b((Ut,Ne)=>{"use strict";var yt=require("path"),Te=Se(),je=ve(),bt=Ie(),xt=process.platform==="win32",Et=/\.(?:com|exe)$/i,St=/node_modules[\\/].bin[\\/][^\\/]+\.cmd$/i;function vt(e){e.file=Te(e);let t=e.file&&bt(e.file);return t?(e.args.unshift(e.file),e.command=t,Te(e)):e.file}function kt(e){if(!xt)return e;let t=vt(e),r=!Et.test(t);if(e.options.forceShell||r){let n=St.test(t);e.command=yt.normalize(e.command),e.command=je.command(e.command),e.args=e.args.map(s=>je.argument(s,n));let o=[e.command].concat(e.args).join(" ");e.args=["/d","/s","/c",`"${o}"`],e.command=process.env.comspec||"cmd.exe",e.options.windowsVerbatimArguments=!0}return e}function Ct(e,t,r){t&&!Array.isArray(t)&&(r=t,t=null),t=t?t.slice(0):[],r=Object.assign({},r);let n={command:e,args:t,options:r,file:void 0,original:{command:e,args:t}};return r.shell?n:kt(n)}Ne.exports=Ct});var Be=b((Vt,Le)=>{"use strict";var _=process.platform==="win32";function G(e,t){return Object.assign(new Error(`${t} ${e.command} ENOENT`),{code:"ENOENT",errno:"ENOENT",syscall:`${t} ${e.command}`,path:e.command,spawnargs:e.args})}function Pt(e,t){if(!_)return;let r=e.emit;e.emit=function(n,o){if(n==="exit"){let s=Fe(o,t);if(s)return r.call(e,"error",s)}return r.apply(e,arguments)}}function Fe(e,t){return _&&e===1&&!t.file?G(t.original,"spawn"):null}function Ot(e,t){return _&&e===1&&!t.file?G(t.original,"spawnSync"):null}Le.exports={hookChildProcess:Pt,verifyENOENT:Fe,verifyENOENTSync:Ot,notFoundError:G}});var Re=b((Kt,N)=>{"use strict";var qe=require("child_process"),U=$e(),V=Be();function Me(e,t,r){let n=U(e,t,r),o=qe.spawn(n.command,n.args,n.options);return V.hookChildProcess(o,n),o}function At(e,t,r){let n=U(e,t,r),o=qe.spawnSync(n.command,n.args,n.options);return o.error=o.error||V.verifyENOENTSync(o.status,n),o}N.exports=Me;N.exports.spawn=Me;N.exports.sync=At;N.exports._parse=U;N.exports._enoent=V});var Ft={};tt(Ft,{installBundledPlugin:()=>_e});module.exports=rt(Ft);var v=require("node:fs"),O=R(require("node:path"),1),De=R(require("node:os"),1);var We=require("node:child_process"),He=R(Re(),1),B=require("node:fs"),I=require("node:fs");var K=require("node:timers/promises");function ze(e){let t=/\n\[(stdout|stderr)\] /g,r=[...e.matchAll(t)];return r.map((n,o)=>n[1]==="stdout"?e.slice(n.index+n[0].length,r[o+1]?.index??e.length):"").join("")}async function Nt(e){if(!e.pid)return e.exitCode!==null||e.signalCode!==null;let r=(e.exitCode!==null||e.signalCode!==null)&&[e.stdin,e.stdout,e.stderr].every(n=>!n||n.closed)?Promise.resolve():new Promise(n=>e.once("close",()=>n()));if(process.platform==="win32"){let n=(0,We.spawnSync)("taskkill",["/PID",String(e.pid),"/T","/F"],{encoding:"utf8",timeout:5e3,maxBuffer:65536,windowsHide:!0});(n.error||n.status!==0)&&e.kill("SIGKILL")}else{try{process.kill(-e.pid,"SIGTERM")}catch{e.kill("SIGTERM")}await Promise.race([r,(0,K.setTimeout)(1e3)]);try{process.kill(-e.pid,"SIGKILL")}catch{e.exitCode===null&&e.signalCode===null&&e.kill("SIGKILL")}}return await Promise.race([r.then(()=>!0),(0,K.setTimeout)(3e3).then(()=>!1)])}async function Je(e){let[t,...r]=e.command;if(!t)throw new Error("Command cannot be empty");let n=(0,I.openSync)(e.logFile,"wx",384);(0,I.writeSync)(n,`start=${new Date().toISOString()}
timeoutMs=${e.timeoutMs}
command=${JSON.stringify(e.command)}
`),(0,I.closeSync)(n);let o=(0,I.createWriteStream)(e.logFile,{flags:"a"}),s=(0,B.statSync)(e.logFile).size,i=!1,p=!1,g=!1,c=!1,u,d,m,y,x=new Promise(l=>{y=l}),h=(0,He.default)(t,r,{cwd:e.cwd,env:e.env,detached:process.platform!=="win32",stdio:["ignore","pipe","pipe"],windowsHide:!0}),k=new Promise(l=>h.once("close",C=>l({exitCode:C}))),A=l=>{l==="timeout"&&(i=!0),l==="output"&&(p=!0),l==="signal"&&(g=!0),m??=Nt(h),m.then(y)},E=()=>A("signal");e.signal?.addEventListener("abort",E,{once:!0}),e.signal?.aborted&&E();let $=setTimeout(()=>A("timeout"),e.timeoutMs),F=(l,C)=>{if(p||d)return;let L=Buffer.from(`
[${l}] `),P=e.maxBytes-s;if(P<=L.length||C.byteLength>P-L.length){let a=Buffer.from(`
[output budget exceeded; process terminated]
`);P>0&&o.write(a.subarray(0,P)),s=e.maxBytes,A("output");return}s+=L.length+C.byteLength,o.write(L),o.write(C)};h.stdout?.on("data",l=>F("stdout",l)),h.stderr?.on("data",l=>F("stderr",l)),h.once("error",l=>{u=l}),o.once("error",l=>{d=l,A("output")});let S,T=!1;try{S=await Promise.race([k,x.then(C=>{T=!C})])}finally{clearTimeout($),e.signal?.removeEventListener("abort",E),m&&(c=!await m),(T||c)&&(h.stdout?.destroy(),h.stderr?.destroy()),await new Promise(l=>{o.closed?l():(o.once("finish",()=>l()),o.once("close",()=>l()),o.end())})}return{exitCode:S?.exitCode??h.exitCode,timedOut:i,outputLimit:p,interrupted:g,closeTimedOut:c,...d?{logError:d.message}:{},...u?{spawnError:u}:{}}}var f=e=>!!e&&typeof e=="object"&&!Array.isArray(e);function $t(e){let t=O.default.join(e,".agents/plugins/marketplace.json"),r=(0,v.lstatSync)(t);if(!r.isFile()||r.isSymbolicLink()||r.size>65536)throw new Error("Invalid bundled plugin marketplace.");let n=JSON.parse((0,v.readFileSync)(t,"utf8"));if(!f(n)||n.name!=="wombat-local"||!Array.isArray(n.plugins))throw new Error("Invalid bundled plugin marketplace.");let o=n.plugins;if(!["wombat","wombat-collection"].every(s=>o.some(i=>f(i)&&i.name===s&&f(i.source)&&i.source.source==="local"&&i.source.path===(s==="wombat"?"./plugin":"./collection-plugin"))))throw new Error("Invalid bundled plugin marketplace.")}async function _e(e,t={}){let r=O.default.resolve(e);$t(r);let n=t.env??process.env,o=t.binary??n.WOMBAT_CODEX_BIN??"codex",s=/\.[cm]?js$/.test(o)?[process.execPath,o]:[o],i=(0,v.mkdtempSync)(O.default.join(De.default.tmpdir(),"wombat-plugin-install-")),p=new AbortController,g=()=>p.abort(),c=t.signal?AbortSignal.any([t.signal,p.signal]):p.signal;process.once("SIGINT",g),process.once("SIGTERM",g);try{let u=async(a,Ge,Ue=s,Ve=[0])=>{let Y=O.default.join(i,a+".log"),w=await Je({command:[...Ue,...Ge],cwd:process.cwd(),env:n,logFile:Y,timeoutMs:t.timeoutMs??2e4,maxBytes:1024*1024,signal:c});if(w.exitCode===null||!Ve.includes(w.exitCode)||w.timedOut||w.outputLimit||w.interrupted||w.closeTimedOut||w.spawnError||w.logError){let Ke=w.spawnError?"executable unavailable":w.timedOut?"timeout":w.outputLimit?"output limit":w.interrupted?"cancelled":w.closeTimedOut?"cleanup incomplete":w.logError?"log write failed":"exit "+w.exitCode;throw new Error(`Codex plugin ${a} failed (${Ke}). Wombat is installed; check Codex, then retry the installer with --plugin-only (PowerShell: -PluginOnly) to use the current runtime without another download.`)}return ze((0,v.readFileSync)(Y,"utf8"))},d=JSON.parse(await u("list",["plugin","list","--marketplace","wombat-local","--json"]));if(!f(d)||!Array.isArray(d.installed))throw new Error("Unrecognized Codex plugin list. Wombat is installed.");let m=d.installed.filter(a=>f(a)&&a.enabled===!0&&typeof a.pluginId=="string"&&["wombat@wombat-local","wombat-collection@wombat-local"].includes(a.pluginId));if(m.length>1)throw new Error("Multiple Wombat plugins are enabled. Choose one in Codex, then rerun the installer.");let y=m.some(a=>f(a)&&a.pluginId==="wombat-collection@wombat-local")?"wombat-collection":"wombat",x=O.default.join(r,y==="wombat"?"plugin":"collection-plugin","wombat-runtime.json"),h=(0,v.lstatSync)(x);if(!h.isFile()||h.isSymbolicLink()||h.size>65536)throw new Error("Invalid bundled runtime requirements.");let k=JSON.parse((0,v.readFileSync)(x,"utf8"));if(!f(k)||k.format!==1||typeof k.version!="string"||typeof k.skillContentHash!="string")throw new Error("Invalid bundled runtime requirements.");let A=JSON.parse(await u("catalogs",["plugin","marketplace","list","--json"]));if(!f(A)||!Array.isArray(A.marketplaces))throw new Error("Unrecognized Codex marketplace list. Wombat is installed.");let E=A.marketplaces.find(a=>f(a)&&a.name==="wombat-local"),$;if(E!==void 0){if(!f(E)||!f(E.marketplaceSource)||E.marketplaceSource.sourceType!=="local"||typeof E.marketplaceSource.source!="string")throw new Error("Existing wombat-local marketplace is not local. Choose its source in Codex before retrying.");O.default.resolve(E.marketplaceSource.source)!==r&&($=E.marketplaceSource.source,await u("unregister",["plugin","marketplace","remove","wombat-local"]))}let F=!1;try{await u("marketplace",["plugin","marketplace","add",r]),F=!0;let a=JSON.parse(await u("install",["plugin","add",y+"@wombat-local","--json"]));if(!f(a)||a.pluginId!==y+"@wombat-local"||typeof a.installedPath!="string"||!O.default.isAbsolute(a.installedPath))throw new Error("Unrecognized Codex plugin installation result. Check the plugin in Codex.")}catch(a){if($&&!c.aborted)try{F&&await u("restore-remove",["plugin","marketplace","remove","wombat-local"]),await u("restore",["plugin","marketplace","add",$])}catch{throw new Error("Plugin installation and marketplace recovery failed. Wombat is installed; restore wombat-local in Codex and rerun the installer.")}throw a}let S=JSON.parse(await u("verify",["setup","--project",process.cwd(),"--json"],[process.execPath,O.default.resolve(r,"../wombat.js")],[0,2])),T=f(S)&&f(S.discovery)?S.discovery:null,l=T&&Array.isArray(T.instances)?T.instances.filter(a=>f(a)&&a.enabled===!0):[],C=l.length===1&&f(l[0])?l[0]:null,P=(f(S)&&Array.isArray(S.runtimeChecks)?S.runtimeChecks:[]).filter(a=>f(a)&&a.path===C?.path);if(!f(S)||S.outputVersion!==1||T?.status!=="available"||C?.name!==y+":wombat"||P.length!==1||!f(P[0])||P[0].status!=="compatible"||P[0].pluginVersion!==k.version||P[0].skillContentHash!==k.skillContentHash)throw new Error("Plugin files are installed, but current-project discovery or runtime verification is incomplete. Run wombat setup in the project; retry only the plugin stage with --plugin-only (PowerShell: -PluginOnly). Installed files and data are retained.");return y}finally{process.off("SIGINT",g),process.off("SIGTERM",g),(0,v.rmSync)(i,{recursive:!0,force:!0})}}if(process.argv[1]==="-"||O.default.basename(process.argv[1]??"")==="install-plugin-bootstrap.ts"){let e=process.argv[2];!e||process.argv.length!==3?(console.error("A bundled plugin marketplace path is required."),process.exitCode=2):_e(e).then(t=>console.log(`Installed and verified ${t} through Codex for ${process.cwd()}. Start a new Codex project conversation and ask $${t}:wombat about your usage. Hook trust is optional for log queries.`),t=>{console.error(t instanceof Error?t.message:"Plugin installation failed. Wombat is installed."),process.exitCode=1})}0&&(module.exports={installBundledPlugin});
// END generated Wombat plugin bootstrap
WOMBAT_PLUGIN_BOOTSTRAP
}
install_plugin
start_app() {
  [ "$open_app" -eq 1 ] || return 0
  [ -z "${tmp:-}" ] || rm -rf "$tmp"
  echo "Starting Wombat..."
  "$launcher" web --open
}
start_app
