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
"use strict";var tt=Object.create;var R=Object.defineProperty;var nt=Object.getOwnPropertyDescriptor;var rt=Object.getOwnPropertyNames;var ot=Object.getPrototypeOf,it=Object.prototype.hasOwnProperty;var S=(e,t)=>()=>(t||e((t={exports:{}}).exports,t),t.exports),st=(e,t)=>{for(var n in t)R(e,n,{get:t[n],enumerable:!0})},ee=(e,t,n,r)=>{if(t&&typeof t=="object"||typeof t=="function")for(let o of rt(t))!it.call(e,o)&&o!==n&&R(e,o,{get:()=>t[o],enumerable:!(r=nt(t,o))||r.enumerable});return e};var G=(e,t,n)=>(n=e!=null?tt(ot(e)):{},ee(t||!e||!e.__esModule?R(n,"default",{value:e,enumerable:!0}):n,e)),at=e=>ee(R({},"__esModule",{value:!0}),e);var ie=S((Ht,oe)=>{oe.exports=re;re.sync=ct;var te=require("fs");function lt(e,t){var n=t.pathExt!==void 0?t.pathExt:process.env.PATHEXT;if(!n||(n=n.split(";"),n.indexOf("")!==-1))return!0;for(var r=0;r<n.length;r++){var o=n[r].toLowerCase();if(o&&e.substr(-o.length).toLowerCase()===o)return!0}return!1}function ne(e,t,n){return!e.isSymbolicLink()&&!e.isFile()?!1:lt(t,n)}function re(e,t,n){te.stat(e,function(r,o){n(r,r?!1:ne(o,e,t))})}function ct(e,t){return ne(te.statSync(e),e,t)}});var ue=S((qt,ce)=>{ce.exports=ae;ae.sync=ut;var se=require("fs");function ae(e,t,n){se.stat(e,function(r,o){n(r,r?!1:le(o,t))})}function ut(e,t){return le(se.statSync(e),t)}function le(e,t){return e.isFile()&&dt(e,t)}function dt(e,t){var n=e.mode,r=e.uid,o=e.gid,s=t.uid!==void 0?t.uid:process.getuid&&process.getuid(),a=t.gid!==void 0?t.gid:process.getgid&&process.getgid(),c=parseInt("100",8),w=parseInt("010",8),l=parseInt("001",8),i=c|w,u=n&l||n&w&&o===a||n&c&&r===s||n&i&&s===0;return u}});var pe=S((Vt,de)=>{var Wt=require("fs"),F;process.platform==="win32"||global.TESTING_WINDOWS?F=ie():F=ue();de.exports=H;H.sync=pt;function H(e,t,n){if(typeof t=="function"&&(n=t,t={}),!n){if(typeof Promise!="function")throw new TypeError("callback not provided");return new Promise(function(r,o){H(e,t||{},function(s,a){s?o(s):r(a)})})}F(e,t||{},function(r,o){r&&(r.code==="EACCES"||t&&t.ignoreErrors)&&(r=null,o=!1),n(r,o)})}function pt(e,t){try{return F.sync(e,t||{})}catch(n){if(t&&t.ignoreErrors||n.code==="EACCES")return!1;throw n}}});var be=S((zt,ye)=>{var U=process.platform==="win32"||process.env.OSTYPE==="cygwin"||process.env.OSTYPE==="msys",me=require("path"),mt=U?";":":",fe=pe(),ge=e=>Object.assign(new Error(`not found: ${e}`),{code:"ENOENT"}),he=(e,t)=>{let n=t.colon||mt,r=e.match(/\//)||U&&e.match(/\\/)?[""]:[...U?[process.cwd()]:[],...(t.path||process.env.PATH||"").split(n)],o=U?t.pathExt||process.env.PATHEXT||".EXE;.CMD;.BAT;.COM":"",s=U?o.split(n):[""];return U&&e.indexOf(".")!==-1&&s[0]!==""&&s.unshift(""),{pathEnv:r,pathExt:s,pathExtExe:o}},we=(e,t,n)=>{typeof t=="function"&&(n=t,t={}),t||(t={});let{pathEnv:r,pathExt:o,pathExtExe:s}=he(e,t),a=[],c=l=>new Promise((i,u)=>{if(l===r.length)return t.all&&a.length?i(a):u(ge(e));let d=r[l],m=/^".*"$/.test(d)?d.slice(1,-1):d,b=me.join(m,e),y=!m&&/^\.[\\\/]/.test(e)?e.slice(0,2)+b:b;i(w(y,l,0))}),w=(l,i,u)=>new Promise((d,m)=>{if(u===o.length)return d(c(i+1));let b=o[u];fe(l+b,{pathExt:s},(y,v)=>{if(!y&&v)if(t.all)a.push(l+b);else return d(l+b);return d(w(l,i,u+1))})});return n?c(0).then(l=>n(null,l),n):c(0)},ft=(e,t)=>{t=t||{};let{pathEnv:n,pathExt:r,pathExtExe:o}=he(e,t),s=[];for(let a=0;a<n.length;a++){let c=n[a],w=/^".*"$/.test(c)?c.slice(1,-1):c,l=me.join(w,e),i=!w&&/^\.[\\\/]/.test(e)?e.slice(0,2)+l:l;for(let u=0;u<r.length;u++){let d=i+r[u];try{if(fe.sync(d,{pathExt:o}))if(t.all)s.push(d);else return d}catch{}}}if(t.all&&s.length)return s;if(t.nothrow)return null;throw ge(e)};ye.exports=we;we.sync=ft});var Se=S((Jt,q)=>{"use strict";var Ee=(e={})=>{let t=e.env||process.env;return(e.platform||process.platform)!=="win32"?"PATH":Object.keys(t).reverse().find(r=>r.toUpperCase()==="PATH")||"Path"};q.exports=Ee;q.exports.default=Ee});var ke=S((Kt,Ce)=>{"use strict";var xe=require("path"),gt=be(),ht=Se();function ve(e,t){let n=e.options.env||process.env,r=process.cwd(),o=e.options.cwd!=null,s=o&&process.chdir!==void 0&&!process.chdir.disabled;if(s)try{process.chdir(e.options.cwd)}catch{}let a;try{a=gt.sync(e.command,{path:n[ht({env:n})],pathExt:t?xe.delimiter:void 0})}catch{}finally{s&&process.chdir(r)}return a&&(a=xe.resolve(o?e.options.cwd:"",a)),a}function wt(e){return ve(e)||ve(e,!0)}Ce.exports=wt});var Ie=S((Xt,V)=>{"use strict";var W=/([()\][%!^"`<>&|;, *?])/g;function yt(e){return e=e.replace(W,"^$1"),e}function bt(e,t){return e=`${e}`,e=e.replace(/(?=(\\+?)?)\1"/g,'$1$1\\"'),e=e.replace(/(?=(\\+?)?)\1$/,"$1$1"),e=`"${e}"`,e=e.replace(W,"^$1"),t&&(e=e.replace(W,"^$1")),e}V.exports.command=yt;V.exports.argument=bt});var Pe=S((Yt,Ae)=>{"use strict";Ae.exports=/^#!(.*)/});var Ne=S((Zt,Oe)=>{"use strict";var Et=Pe();Oe.exports=(e="")=>{let t=e.match(Et);if(!t)return null;let[n,r]=t[0].replace(/#! ?/,"").split(" "),o=n.split("/").pop();return o==="env"?r:r?`${o} ${r}`:o}});var Le=S((Qt,Te)=>{"use strict";var z=require("fs"),St=Ne();function xt(e){let n=Buffer.alloc(150),r;try{r=z.openSync(e,"r"),z.readSync(r,n,0,150,0),z.closeSync(r)}catch{}return St(n.toString())}Te.exports=xt});var $e=S((en,Ue)=>{"use strict";var vt=require("path"),_e=ke(),je=Ie(),Ct=Le(),kt=process.platform==="win32",It=/\.(?:com|exe)$/i,At=/node_modules[\\/].bin[\\/][^\\/]+\.cmd$/i;function Pt(e){e.file=_e(e);let t=e.file&&Ct(e.file);return t?(e.args.unshift(e.file),e.command=t,_e(e)):e.file}function Ot(e){if(!kt)return e;let t=Pt(e),n=!It.test(t);if(e.options.forceShell||n){let r=At.test(t);e.command=vt.normalize(e.command),e.command=je.command(e.command),e.args=e.args.map(s=>je.argument(s,r));let o=[e.command].concat(e.args).join(" ");e.args=["/d","/s","/c",`"${o}"`],e.command=process.env.comspec||"cmd.exe",e.options.windowsVerbatimArguments=!0}return e}function Nt(e,t,n){t&&!Array.isArray(t)&&(n=t,t=null),t=t?t.slice(0):[],n=Object.assign({},n);let r={command:e,args:t,options:n,file:void 0,original:{command:e,args:t}};return n.shell?r:Ot(r)}Ue.exports=Nt});var Re=S((tn,Me)=>{"use strict";var J=process.platform==="win32";function K(e,t){return Object.assign(new Error(`${t} ${e.command} ENOENT`),{code:"ENOENT",errno:"ENOENT",syscall:`${t} ${e.command}`,path:e.command,spawnargs:e.args})}function Tt(e,t){if(!J)return;let n=e.emit;e.emit=function(r,o){if(r==="exit"){let s=De(o,t);if(s)return n.call(e,"error",s)}return n.apply(e,arguments)}}function De(e,t){return J&&e===1&&!t.file?K(t.original,"spawn"):null}function Lt(e,t){return J&&e===1&&!t.file?K(t.original,"spawnSync"):null}Me.exports={hookChildProcess:Tt,verifyENOENT:De,verifyENOENTSync:Lt,notFoundError:K}});var Ge=S((nn,$)=>{"use strict";var Fe=require("child_process"),X=$e(),Y=Re();function Be(e,t,n){let r=X(e,t,n),o=Fe.spawn(r.command,r.args,r.options);return Y.hookChildProcess(o,r),o}function _t(e,t,n){let r=X(e,t,n),o=Fe.spawnSync(r.command,r.args,r.options);return o.error=o.error||Y.verifyENOENTSync(o.status,r),o}$.exports=Be;$.exports.spawn=Be;$.exports.sync=_t;$.exports._parse=X;$.exports._enoent=Y});var Bt={};st(Bt,{installBundledPlugin:()=>Ke});module.exports=at(Bt);var E=require("node:fs"),x=G(require("node:path"),1),ze=G(require("node:os"),1);var He=require("node:child_process"),qe=G(Ge(),1),M=require("node:fs"),O=require("node:fs");var Z=require("node:timers/promises");function We(e){let t=/\n\[(stdout|stderr)\] /g,n=[...e.matchAll(t)];return n.map((r,o)=>r[1]==="stdout"?e.slice(r.index+r[0].length,n[o+1]?.index??e.length):"").join("")}async function Dt(e){if(!e.pid)return e.exitCode!==null||e.signalCode!==null;let n=(e.exitCode!==null||e.signalCode!==null)&&[e.stdin,e.stdout,e.stderr].every(r=>!r||r.closed)?Promise.resolve():new Promise(r=>e.once("close",()=>r()));if(process.platform==="win32"){let r=(0,He.spawnSync)("taskkill",["/PID",String(e.pid),"/T","/F"],{encoding:"utf8",timeout:5e3,maxBuffer:65536,windowsHide:!0});(r.error||r.status!==0)&&e.kill("SIGKILL")}else{try{process.kill(-e.pid,"SIGTERM")}catch{e.kill("SIGTERM")}await Promise.race([n,(0,Z.setTimeout)(1e3)]);try{process.kill(-e.pid,"SIGKILL")}catch{e.exitCode===null&&e.signalCode===null&&e.kill("SIGKILL")}}return await Promise.race([n.then(()=>!0),(0,Z.setTimeout)(3e3).then(()=>!1)])}async function Ve(e){let[t,...n]=e.command;if(!t)throw new Error("Command cannot be empty");let r=(0,O.openSync)(e.logFile,"wx",384);(0,O.writeSync)(r,`start=${new Date().toISOString()}
timeoutMs=${e.timeoutMs}
command=${JSON.stringify(e.command)}
`),(0,O.closeSync)(r);let o=(0,O.createWriteStream)(e.logFile,{flags:"a"}),s=(0,M.statSync)(e.logFile).size,a=!1,c=!1,w=!1,l=!1,i,u,d,m,b=new Promise(p=>{m=p}),y=(0,qe.default)(t,n,{cwd:e.cwd,env:e.env,detached:process.platform!=="win32",stdio:["ignore","pipe","pipe"],windowsHide:!0}),v=new Promise(p=>y.once("close",k=>p({exitCode:k}))),I=p=>{p==="timeout"&&(a=!0),p==="output"&&(c=!0),p==="signal"&&(w=!0),d??=Dt(y),d.then(m)},P=()=>I("signal");e.signal?.addEventListener("abort",P,{once:!0}),e.signal?.aborted&&P();let C=setTimeout(()=>I("timeout"),e.timeoutMs),L=(p,k)=>{if(c||u)return;let j=Buffer.from(`
[${p}] `),N=e.maxBytes-s;if(N<=j.length||k.byteLength>N-j.length){let h=Buffer.from(`
[output budget exceeded; process terminated]
`);N>0&&o.write(h.subarray(0,N)),s=e.maxBytes,I("output");return}s+=j.length+k.byteLength,o.write(j),o.write(k)};y.stdout?.on("data",p=>L("stdout",p)),y.stderr?.on("data",p=>L("stderr",p)),y.once("error",p=>{i=p}),o.once("error",p=>{u=p,I("output")});let A,_=!1;try{A=await Promise.race([v,b.then(k=>{_=!k})])}finally{clearTimeout(C),e.signal?.removeEventListener("abort",P),d&&(l=!await d),(_||l)&&(y.stdout?.destroy(),y.stderr?.destroy()),await new Promise(p=>{o.closed?p():(o.once("finish",()=>p()),o.once("close",()=>p()),o.end())})}return{exitCode:A?.exitCode??y.exitCode,timedOut:a,outputLimit:c,interrupted:w,closeTimedOut:l,...u?{logError:u.message}:{},...i?{spawnError:i}:{}}}var g=e=>!!e&&typeof e=="object"&&!Array.isArray(e),T=class extends Error{code;recovery;constructor(t,n,r){super(n),this.code=t,this.recovery=r}},D=e=>typeof e=="string"&&/^[A-Z][A-Z0-9_]{0,63}$/.test(e)?e:void 0,Je=e=>"'"+e.replaceAll("'",process.platform==="win32"?"''":"'\\''")+"'";function Mt(e,t,n){let r=`Run only the read-only check: wombat setup --project ${Je(process.cwd())} --json. Do not reinstall the runtime.`,o=(i,u,d=r)=>new T(i,u,d);if(!g(e)||e.outputVersion!==1||!g(e.discovery)||!Array.isArray(e.discovery.instances)||!Array.isArray(e.runtimeChecks))return o("SETUP_INVALID_RESPONSE","The verification response is invalid.");let s=e.discovery;if(s.status!=="available"){let i=D(s.errorCode)??{missing:"PLUGIN_NOT_DISCOVERED",disabled:"PLUGIN_DISABLED",ambiguous:"PLUGIN_AMBIGUOUS"}[String(s.status)]??"PLUGIN_DISCOVERY_UNAVAILABLE",u={LOCAL_ACCESS_DENIED:"Local permissions prevented Codex plugin discovery. Run the check in a terminal allowed to access Codex.",CODEX_DISCONNECTED:"Codex disconnected during plugin discovery.",CODEX_TIMEOUT:"Codex plugin discovery timed out.",SETUP_TIMEOUT:"Plugin discovery exceeded the setup deadline.",CANCELLED:"Plugin discovery did not complete before cancellation or the setup deadline.",SKILL_PARSE_ERROR:"Codex reported Skill parsing errors in this project.",PLUGIN_NOT_DISCOVERED:"Codex did not discover the installed Wombat Skill in this project.",PLUGIN_DISABLED:"The Wombat Skill is disabled in this project.",PLUGIN_AMBIGUOUS:"More than one Wombat Skill is enabled in this project."},d=["PLUGIN_DISABLED","PLUGIN_AMBIGUOUS","PLUGIN_NOT_DISCOVERED","SKILL_PARSE_ERROR"].includes(i)?"Check this project's plugin settings and Skill errors in Codex first. ":"";return o(i,u[i]??"Codex plugin discovery is unavailable.",d+r)}let a=Array.isArray(s.instances)?s.instances.filter(i=>g(i)&&i.enabled===!0):[],c=a.length===1&&g(a[0])?a[0]:null;if(!c||c.name!==t+":wombat"||typeof c.path!="string"||!x.default.isAbsolute(c.path))return o("PLUGIN_SELECTION_MISMATCH","Codex selected a different or ambiguous Wombat Skill.","Select one intended Wombat plugin in Codex. "+r);let w=e.runtimeChecks.filter(i=>g(i)&&i.path===c.path);if(w.length!==1||!g(w[0])){let i=Array.isArray(e.errorCodes)&&e.errorCodes.some(u=>["SETUP_TIMEOUT","CANCELLED"].includes(String(u)));return o(i?"SETUP_TIMEOUT":"PLUGIN_RUNTIME_CHECK_MISSING",i?"The setup deadline expired before runtime compatibility could be verified.":"No unique runtime check was returned for the discovered Wombat Skill.")}let l=w[0];if(l.status!=="compatible"){let i=["modified","incompatible","unmanaged"].includes(String(l.status))?"Repair only the plugin with --plugin-only (PowerShell: -PluginOnly); keep any custom prefix. Do not reinstall the runtime.":r;return o(D(l.errorCode)??{modified:"SKILL_RESOURCE_CHANGED",incompatible:"SKILL_RUNTIME_MISMATCH",unmanaged:"PLUGIN_UNMANAGED"}[String(l.status)]??"PLUGIN_RUNTIME_UNAVAILABLE",`The discovered Wombat Skill runtime check is ${["modified","incompatible","unmanaged","unavailable"].includes(String(l.status))?l.status:"invalid"}.`,i)}if(l.pluginVersion!==n.version){let i=typeof l.pluginVersion=="string"&&/^[0-9A-Za-z][0-9A-Za-z._-]{0,63}$/.test(l.pluginVersion)?l.pluginVersion:"unknown";return o("PLUGIN_VERSION_MISMATCH",`Expected plugin ${n.version}; Codex discovered ${i}.`,"Refresh only the plugin with --plugin-only (PowerShell: -PluginOnly); keep any custom prefix. Do not reinstall the runtime.")}if(l.skillContentHash!==n.skillContentHash)return o("PLUGIN_CONTENT_MISMATCH","The discovered Skill content differs from the installed runtime bundle.","Repair only the plugin with --plugin-only (PowerShell: -PluginOnly); keep any custom prefix. Do not reinstall the runtime.")}function Rt(e){if(!g(e))return{};let t=new Set(["available","unavailable","missing","disabled","ambiguous","compatible","modified","incompatible","unmanaged"]),n=o=>typeof o=="string"&&o.length<=4096&&!/[\x00-\x1f\x7f]/.test(o)?o:null,r=g(e.discovery)?e.discovery:{};return{discovery:{status:t.has(String(r.status))?r.status:null,errorCode:D(r.errorCode)??null,instances:Array.isArray(r.instances)?r.instances.slice(0,32).filter(g).map(o=>({name:n(o.name),path:n(o.path),enabled:o.enabled===!0})):[]},runtimeVersion:n(e.runtimeVersion),runtimeChecks:Array.isArray(e.runtimeChecks)?e.runtimeChecks.slice(0,32).filter(g).map(o=>({path:n(o.path),status:t.has(String(o.status))?o.status:null,pluginVersion:n(o.pluginVersion),skillContentHash:typeof o.skillContentHash=="string"&&/^[a-f0-9]{64}$/.test(o.skillContentHash)?o.skillContentHash:null,missingCapabilities:Array.isArray(o.missingCapabilities)?o.missingCapabilities.slice(0,32).map(n).filter(Boolean):[],errorCode:D(o.errorCode)??null})):[],errorCodes:Array.isArray(e.errorCodes)?e.errorCodes.slice(0,32).map(D).filter(Boolean):[]}}function Ft(e){let t=x.default.join(e,".agents/plugins/marketplace.json"),n=(0,E.lstatSync)(t);if(!n.isFile()||n.isSymbolicLink()||n.size>65536)throw new Error("Invalid bundled plugin marketplace.");let r=JSON.parse((0,E.readFileSync)(t,"utf8"));if(!g(r)||r.name!=="wombat-local"||!Array.isArray(r.plugins))throw new Error("Invalid bundled plugin marketplace.");let o=r.plugins;if(!["wombat","wombat-collection"].every(s=>o.some(a=>g(a)&&a.name===s&&g(a.source)&&a.source.source==="local"&&a.source.path===(s==="wombat"?"./plugin":"./collection-plugin"))))throw new Error("Invalid bundled plugin marketplace.")}async function Ke(e,t={}){let n=x.default.resolve(e);Ft(n);let r=t.env??process.env,o=t.binary??r.WOMBAT_CODEX_BIN??"codex",s=/\.[cm]?js$/.test(o)?[process.execPath,o]:[o],a=(0,E.mkdtempSync)(x.default.join(ze.default.tmpdir(),"wombat-plugin-install-")),c=new AbortController,w=()=>c.abort(),l=t.signal?AbortSignal.any([t.signal,c.signal]):c.signal,i="list",u=!1,d={};process.once("SIGINT",w),process.once("SIGTERM",w);try{let m=async(h,B,Xe=s,Ye=[0])=>{i=h;let Q=x.default.join(a,i+".log"),f=await Ve({command:[...Xe,...B],cwd:process.cwd(),env:{...r,WOMBAT_CODEX_BIN:o},logFile:Q,timeoutMs:t.timeoutMs??2e4,maxBytes:1024*1024,signal:l});if(f.exitCode===null||!Ye.includes(f.exitCode)||f.timedOut||f.outputLimit||f.interrupted||f.closeTimedOut||f.spawnError||f.logError){let Ze=f.spawnError?"executable unavailable":f.timedOut?"timeout":f.outputLimit?"output limit":f.interrupted?"cancelled":f.closeTimedOut?"cleanup incomplete":f.logError?"log write failed":"exit "+f.exitCode,Qe=f.spawnError?("code"in f.spawnError?D(f.spawnError.code):void 0)??"EXECUTABLE_UNAVAILABLE":f.timedOut?"TIMEOUT":f.outputLimit?"OUTPUT_LIMIT":f.interrupted?"CANCELLED":f.closeTimedOut?"CLEANUP_INCOMPLETE":f.logError?"LOG_WRITE_FAILED":"COMMAND_FAILED",et=i==="verify"?`Run only the read-only check: wombat setup --project ${Je(process.cwd())} --json. Do not reinstall the runtime.`:"Retry only the plugin stage with --plugin-only (PowerShell: -PluginOnly); keep any custom prefix. Do not reinstall the runtime.";throw new T(Qe,`Codex plugin ${i} failed (${Ze}).`,et)}return We((0,E.readFileSync)(Q,"utf8"))},b=JSON.parse(await m("list",["plugin","list","--marketplace","wombat-local","--json"]));if(!g(b)||!Array.isArray(b.installed))throw new Error("Unrecognized Codex plugin list. Wombat is installed.");let y=b.installed.filter(h=>g(h)&&h.enabled===!0&&typeof h.pluginId=="string"&&["wombat@wombat-local","wombat-collection@wombat-local"].includes(h.pluginId));if(y.length>1)throw new Error("Multiple Wombat plugins are enabled. Choose one in Codex, then rerun the installer.");let v=y.some(h=>g(h)&&h.pluginId==="wombat-collection@wombat-local")?"wombat-collection":"wombat";i="requirements";let I=x.default.join(n,v==="wombat"?"plugin":"collection-plugin","wombat-runtime.json"),P=(0,E.lstatSync)(I);if(!P.isFile()||P.isSymbolicLink()||P.size>65536)throw new Error("Invalid bundled runtime requirements.");let C=JSON.parse((0,E.readFileSync)(I,"utf8"));if(!g(C)||C.format!==1||typeof C.version!="string"||!/^[0-9A-Za-z][0-9A-Za-z._-]{0,63}$/.test(C.version)||typeof C.skillContentHash!="string"||!/^[a-f0-9]{64}$/.test(C.skillContentHash))throw new Error("Invalid bundled runtime requirements.");d={plugin:v,expected:{pluginVersion:C.version,skillContentHash:C.skillContentHash}};let L=JSON.parse(await m("catalogs",["plugin","marketplace","list","--json"]));if(!g(L)||!Array.isArray(L.marketplaces))throw new Error("Unrecognized Codex marketplace list. Wombat is installed.");let A=L.marketplaces.find(h=>g(h)&&h.name==="wombat-local"),_;if(A!==void 0){if(!g(A)||!g(A.marketplaceSource)||A.marketplaceSource.sourceType!=="local"||typeof A.marketplaceSource.source!="string")throw new Error("Existing wombat-local marketplace is not local. Choose its source in Codex before retrying.");x.default.resolve(A.marketplaceSource.source)!==n&&(_=A.marketplaceSource.source,await m("unregister",["plugin","marketplace","remove","wombat-local"]))}let p=!1;try{await m("marketplace",["plugin","marketplace","add",n]),p=!0;let h=JSON.parse(await m("install",["plugin","add",v+"@wombat-local","--json"]));if(!g(h)||h.pluginId!==v+"@wombat-local"||typeof h.installedPath!="string"||!x.default.isAbsolute(h.installedPath))throw new Error("Unrecognized Codex plugin installation result. Check the plugin in Codex.")}catch(h){let B=i;if(_&&!l.aborted)try{p&&await m("restore-remove",["plugin","marketplace","remove","wombat-local"]),await m("restore",["plugin","marketplace","add",_])}catch{throw new Error("Plugin installation and marketplace recovery failed. Wombat is installed; restore wombat-local in Codex and rerun the installer.")}throw i=B,h}let k,j=await m("verify",["setup","--project",process.cwd(),"--json"],[process.execPath,x.default.resolve(n,"../wombat.js")],[0,2]);try{k=JSON.parse(j)}catch{throw new T("SETUP_INVALID_RESPONSE","The verification response is not valid JSON.","Run wombat setup --json to check the installed runtime. Do not reinstall the runtime.")}d={...d,...Rt(k)};let N=Mt(k,v,C);if(N)throw N;return u=!0,v}catch(m){for(let P of(0,E.readdirSync)(a))(0,E.rmSync)(x.default.join(a,P),{force:!0});let b=m instanceof T?m:new T("PLUGIN_INSTALL_FAILED",m instanceof SyntaxError?"Codex returned invalid JSON.":m instanceof Error?m.message:"Plugin installation failed.","Check this stage before retrying only the plugin installation with --plugin-only (PowerShell: -PluginOnly)."),y=x.default.join(a,"diagnostic.json"),v={outputVersion:1,stage:i,code:b.code,project:process.cwd(),checkedAt:new Date().toISOString(),...d},I=!1;try{(0,E.writeFileSync)(y,JSON.stringify(v,null,2)+`
`,{mode:384}),I=!0}catch{}throw new Error(`Wombat is installed. Plugin ${i} failed [${b.code}]: ${b.message} ${b.recovery} Installed files and data are retained.${I?" Diagnostic summary: "+y:" Diagnostic summary could not be saved."}`)}finally{process.off("SIGINT",w),process.off("SIGTERM",w),u&&(0,E.rmSync)(a,{recursive:!0,force:!0})}}if(process.argv[1]==="-"||x.default.basename(process.argv[1]??"")==="install-plugin-bootstrap.ts"){let e=process.argv[2];!e||process.argv.length!==3?(console.error("A bundled plugin marketplace path is required."),process.exitCode=2):Ke(e).then(t=>console.log(`Installed and verified ${t} through Codex for ${process.cwd()}. Start a new Codex project conversation and ask $${t}:wombat about your usage. Hook trust is optional for log queries.`),t=>{console.error(t instanceof Error?t.message:"Plugin installation failed. Wombat is installed."),process.exitCode=1})}0&&(module.exports={installBundledPlugin});
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
