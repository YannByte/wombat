#!/bin/sh
set -eu

repo="YannByte/wombat"
version="latest"
prefix="${WOMBAT_INSTALL_PREFIX:-$HOME/.local}"
base_url=""
modify_path=1
install_plugin_requested=0
plugin_only=0

while [ "$#" -gt 0 ]; do
  case "$1" in
    --version) version=${2:?missing version}; shift 2 ;;
    --prefix) prefix=${2:?missing prefix}; shift 2 ;;
    --base-url) base_url=${2:?missing base URL}; shift 2 ;;
    --no-modify-path) modify_path=0; shift ;;
    --plugin) install_plugin_requested=1; shift ;;
    --plugin-only) install_plugin_requested=1; plugin_only=1; shift ;;
    -h|--help) echo "Usage: install.sh [--version VERSION|latest] [--prefix PATH] [--base-url URL] [--no-modify-path] [--plugin|--plugin-only]"; exit 0 ;;
    *) echo "Unknown argument: $1" >&2; exit 2 ;;
  esac
done

install_root="$prefix/lib/wombat"
bin_dir="$prefix/bin"
launcher="$bin_dir/wombat"
if [ "$plugin_only" -eq 1 ]; then
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
"use strict";var nt=Object.create;var M=Object.defineProperty;var rt=Object.getOwnPropertyDescriptor;var ot=Object.getOwnPropertyNames;var it=Object.getPrototypeOf,st=Object.prototype.hasOwnProperty;var x=(e,t)=>()=>(t||e((t={exports:{}}).exports,t),t.exports),at=(e,t)=>{for(var n in t)M(e,n,{get:t[n],enumerable:!0})},te=(e,t,n,r)=>{if(t&&typeof t=="object"||typeof t=="function")for(let o of ot(t))!st.call(e,o)&&o!==n&&M(e,o,{get:()=>t[o],enumerable:!(r=rt(t,o))||r.enumerable});return e};var V=(e,t,n)=>(n=e!=null?nt(it(e)):{},te(t||!e||!e.__esModule?M(n,"default",{value:e,enumerable:!0}):n,e)),lt=e=>te(M({},"__esModule",{value:!0}),e);var se=x((Vt,ie)=>{ie.exports=oe;oe.sync=ut;var ne=require("fs");function ct(e,t){var n=t.pathExt!==void 0?t.pathExt:process.env.PATHEXT;if(!n||(n=n.split(";"),n.indexOf("")!==-1))return!0;for(var r=0;r<n.length;r++){var o=n[r].toLowerCase();if(o&&e.substr(-o.length).toLowerCase()===o)return!0}return!1}function re(e,t,n){return!e.isSymbolicLink()&&!e.isFile()?!1:ct(t,n)}function oe(e,t,n){ne.stat(e,function(r,o){n(r,r?!1:re(o,e,t))})}function ut(e,t){return re(ne.statSync(e),e,t)}});var de=x((Wt,ue)=>{ue.exports=le;le.sync=dt;var ae=require("fs");function le(e,t,n){ae.stat(e,function(r,o){n(r,r?!1:ce(o,t))})}function dt(e,t){return ce(ae.statSync(e),t)}function ce(e,t){return e.isFile()&&pt(e,t)}function pt(e,t){var n=e.mode,r=e.uid,o=e.gid,s=t.uid!==void 0?t.uid:process.getuid&&process.getuid(),a=t.gid!==void 0?t.gid:process.getgid&&process.getgid(),c=parseInt("100",8),h=parseInt("010",8),l=parseInt("001",8),i=c|h,u=n&l||n&h&&o===a||n&c&&r===s||n&i&&s===0;return u}});var me=x((Ht,pe)=>{var qt=require("fs"),F;process.platform==="win32"||global.TESTING_WINDOWS?F=se():F=de();pe.exports=W;W.sync=mt;function W(e,t,n){if(typeof t=="function"&&(n=t,t={}),!n){if(typeof Promise!="function")throw new TypeError("callback not provided");return new Promise(function(r,o){W(e,t||{},function(s,a){s?o(s):r(a)})})}F(e,t||{},function(r,o){r&&(r.code==="EACCES"||t&&t.ignoreErrors)&&(r=null,o=!1),n(r,o)})}function mt(e,t){try{return F.sync(e,t||{})}catch(n){if(t&&t.ignoreErrors||n.code==="EACCES")return!1;throw n}}});var Se=x((zt,Ee)=>{var j=process.platform==="win32"||process.env.OSTYPE==="cygwin"||process.env.OSTYPE==="msys",fe=require("path"),ft=j?";":":",ge=me(),he=e=>Object.assign(new Error(`not found: ${e}`),{code:"ENOENT"}),ye=(e,t)=>{let n=t.colon||ft,r=e.match(/\//)||j&&e.match(/\\/)?[""]:[...j?[process.cwd()]:[],...(t.path||process.env.PATH||"").split(n)],o=j?t.pathExt||process.env.PATHEXT||".EXE;.CMD;.BAT;.COM":"",s=j?o.split(n):[""];return j&&e.indexOf(".")!==-1&&s[0]!==""&&s.unshift(""),{pathEnv:r,pathExt:s,pathExtExe:o}},we=(e,t,n)=>{typeof t=="function"&&(n=t,t={}),t||(t={});let{pathEnv:r,pathExt:o,pathExtExe:s}=ye(e,t),a=[],c=l=>new Promise((i,u)=>{if(l===r.length)return t.all&&a.length?i(a):u(he(e));let d=r[l],m=/^".*"$/.test(d)?d.slice(1,-1):d,w=fe.join(m,e),E=!m&&/^\.[\\\/]/.test(e)?e.slice(0,2)+w:w;i(h(E,l,0))}),h=(l,i,u)=>new Promise((d,m)=>{if(u===o.length)return d(c(i+1));let w=o[u];ge(l+w,{pathExt:s},(E,v)=>{if(!E&&v)if(t.all)a.push(l+w);else return d(l+w);return d(h(l,i,u+1))})});return n?c(0).then(l=>n(null,l),n):c(0)},gt=(e,t)=>{t=t||{};let{pathEnv:n,pathExt:r,pathExtExe:o}=ye(e,t),s=[];for(let a=0;a<n.length;a++){let c=n[a],h=/^".*"$/.test(c)?c.slice(1,-1):c,l=fe.join(h,e),i=!h&&/^\.[\\\/]/.test(e)?e.slice(0,2)+l:l;for(let u=0;u<r.length;u++){let d=i+r[u];try{if(ge.sync(d,{pathExt:o}))if(t.all)s.push(d);else return d}catch{}}}if(t.all&&s.length)return s;if(t.nothrow)return null;throw he(e)};Ee.exports=we;we.sync=gt});var xe=x((Jt,q)=>{"use strict";var be=(e={})=>{let t=e.env||process.env;return(e.platform||process.platform)!=="win32"?"PATH":Object.keys(t).reverse().find(r=>r.toUpperCase()==="PATH")||"Path"};q.exports=be;q.exports.default=be});var Ae=x((Kt,ke)=>{"use strict";var Ce=require("path"),ht=Se(),yt=xe();function ve(e,t){let n=e.options.env||process.env,r=process.cwd(),o=e.options.cwd!=null,s=o&&process.chdir!==void 0&&!process.chdir.disabled;if(s)try{process.chdir(e.options.cwd)}catch{}let a;try{a=ht.sync(e.command,{path:n[yt({env:n})],pathExt:t?Ce.delimiter:void 0})}catch{}finally{s&&process.chdir(r)}return a&&(a=Ce.resolve(o?e.options.cwd:"",a)),a}function wt(e){return ve(e)||ve(e,!0)}ke.exports=wt});var Ie=x((Xt,z)=>{"use strict";var H=/([()\][%!^"`<>&|;, *?])/g;function Et(e){return e=e.replace(H,"^$1"),e}function St(e,t){return e=`${e}`,e=e.replace(/(?=(\\+?)?)\1"/g,'$1$1\\"'),e=e.replace(/(?=(\\+?)?)\1$/,"$1$1"),e=`"${e}"`,e=e.replace(H,"^$1"),t&&(e=e.replace(H,"^$1")),e}z.exports.command=Et;z.exports.argument=St});var Oe=x((Zt,Pe)=>{"use strict";Pe.exports=/^#!(.*)/});var Le=x((Yt,Ne)=>{"use strict";var bt=Oe();Ne.exports=(e="")=>{let t=e.match(bt);if(!t)return null;let[n,r]=t[0].replace(/#! ?/,"").split(" "),o=n.split("/").pop();return o==="env"?r:r?`${o} ${r}`:o}});var _e=x((Qt,Te)=>{"use strict";var J=require("fs"),xt=Le();function Ct(e){let n=Buffer.alloc(150),r;try{r=J.openSync(e,"r"),J.readSync(r,n,0,150,0),J.closeSync(r)}catch{}return xt(n.toString())}Te.exports=Ct});var De=x((en,$e)=>{"use strict";var vt=require("path"),Re=Ae(),je=Ie(),kt=_e(),At=process.platform==="win32",It=/\.(?:com|exe)$/i,Pt=/node_modules[\\/].bin[\\/][^\\/]+\.cmd$/i;function Ot(e){e.file=Re(e);let t=e.file&&kt(e.file);return t?(e.args.unshift(e.file),e.command=t,Re(e)):e.file}function Nt(e){if(!At)return e;let t=Ot(e),n=!It.test(t);if(e.options.forceShell||n){let r=Pt.test(t);e.command=vt.normalize(e.command),e.command=je.command(e.command),e.args=e.args.map(s=>je.argument(s,r));let o=[e.command].concat(e.args).join(" ");e.args=["/d","/s","/c",`"${o}"`],e.command=process.env.comspec||"cmd.exe",e.options.windowsVerbatimArguments=!0}return e}function Lt(e,t,n){t&&!Array.isArray(t)&&(n=t,t=null),t=t?t.slice(0):[],n=Object.assign({},n);let r={command:e,args:t,options:n,file:void 0,original:{command:e,args:t}};return n.shell?r:Nt(r)}$e.exports=Lt});var Fe=x((tn,Me)=>{"use strict";var K=process.platform==="win32";function X(e,t){return Object.assign(new Error(`${t} ${e.command} ENOENT`),{code:"ENOENT",errno:"ENOENT",syscall:`${t} ${e.command}`,path:e.command,spawnargs:e.args})}function Tt(e,t){if(!K)return;let n=e.emit;e.emit=function(r,o){if(r==="exit"){let s=Ue(o,t);if(s)return n.call(e,"error",s)}return n.apply(e,arguments)}}function Ue(e,t){return K&&e===1&&!t.file?X(t.original,"spawn"):null}function _t(e,t){return K&&e===1&&!t.file?X(t.original,"spawnSync"):null}Me.exports={hookChildProcess:Tt,verifyENOENT:Ue,verifyENOENTSync:_t,notFoundError:X}});var Ve=x((nn,$)=>{"use strict";var Be=require("child_process"),Z=De(),Y=Fe();function Ge(e,t,n){let r=Z(e,t,n),o=Be.spawn(r.command,r.args,r.options);return Y.hookChildProcess(o,r),o}function Rt(e,t,n){let r=Z(e,t,n),o=Be.spawnSync(r.command,r.args,r.options);return o.error=o.error||Y.verifyENOENTSync(o.status,r),o}$.exports=Ge;$.exports.spawn=Ge;$.exports.sync=Rt;$.exports._parse=Z;$.exports._enoent=Y});var Bt={};at(Bt,{installBundledPlugin:()=>Xe});module.exports=lt(Bt);var S=require("node:fs"),C=V(require("node:path"),1),Je=V(require("node:os"),1);var We=require("node:child_process"),qe=V(Ve(),1),B=require("node:fs"),O=require("node:fs");var Q=require("node:timers/promises");function He(e){let t=/\n\[(stdout|stderr)\] /g,n=[...e.matchAll(t)];return n.map((r,o)=>r[1]==="stdout"?e.slice(r.index+r[0].length,n[o+1]?.index??e.length):"").join("")}async function Dt(e){if(!e.pid)return e.exitCode!==null||e.signalCode!==null;let n=(e.exitCode!==null||e.signalCode!==null)&&[e.stdin,e.stdout,e.stderr].every(r=>!r||r.closed)?Promise.resolve():new Promise(r=>e.once("close",()=>r()));if(process.platform==="win32"){let r=(0,We.spawnSync)("taskkill",["/PID",String(e.pid),"/T","/F"],{encoding:"utf8",timeout:5e3,maxBuffer:65536,windowsHide:!0});(r.error||r.status!==0)&&e.kill("SIGKILL")}else{try{process.kill(-e.pid,"SIGTERM")}catch{e.kill("SIGTERM")}await Promise.race([n,(0,Q.setTimeout)(1e3)]);try{process.kill(-e.pid,"SIGKILL")}catch{e.exitCode===null&&e.signalCode===null&&e.kill("SIGKILL")}}return await Promise.race([n.then(()=>!0),(0,Q.setTimeout)(3e3).then(()=>!1)])}async function ze(e){let[t,...n]=e.command;if(!t)throw new Error("Command cannot be empty");let r=(0,O.openSync)(e.logFile,"wx",384);(0,O.writeSync)(r,`start=${new Date().toISOString()}
timeoutMs=${e.timeoutMs}
command=${JSON.stringify(e.command)}
`),(0,O.closeSync)(r);let o=(0,O.createWriteStream)(e.logFile,{flags:"a"}),s=(0,B.statSync)(e.logFile).size,a=!1,c=!1,h=!1,l=!1,i,u,d,m,w=new Promise(p=>{m=p}),E=(0,qe.default)(t,n,{cwd:e.cwd,env:e.env,detached:process.platform!=="win32",stdio:["ignore","pipe","pipe"],windowsHide:!0}),v=new Promise(p=>E.once("close",b=>p({exitCode:b}))),A=p=>{p==="timeout"&&(a=!0),p==="output"&&(c=!0),p==="signal"&&(h=!0),d??=Dt(E),d.then(m)},P=()=>A("signal");e.signal?.addEventListener("abort",P,{once:!0}),e.signal?.aborted&&P();let k=setTimeout(()=>A("timeout"),e.timeoutMs),T=(p,b)=>{if(c||u)return;let R=Buffer.from(`
[${p}] `),N=e.maxBytes-s;if(N<=R.length||b.byteLength>N-R.length){let U=Buffer.from(`
[output budget exceeded; process terminated]
`);N>0&&o.write(U.subarray(0,N)),s=e.maxBytes,A("output");return}s+=R.length+b.byteLength,o.write(R),o.write(b)};E.stdout?.on("data",p=>T("stdout",p)),E.stderr?.on("data",p=>T("stderr",p)),E.once("error",p=>{i=p}),o.once("error",p=>{u=p,A("output")});let I,_=!1;try{I=await Promise.race([v,w.then(b=>{_=!b})])}finally{clearTimeout(k),e.signal?.removeEventListener("abort",P),d&&(l=!await d),(_||l)&&(E.stdout?.destroy(),E.stderr?.destroy()),await new Promise(p=>{o.closed?p():(o.once("finish",()=>p()),o.once("close",()=>p()),o.end())})}return{exitCode:I?.exitCode??E.exitCode,timedOut:a,outputLimit:c,interrupted:h,closeTimedOut:l,...u?{logError:u.message}:{},...i?{spawnError:i}:{}}}var g=e=>!!e&&typeof e=="object"&&!Array.isArray(e),L=class extends Error{code;recovery;constructor(t,n,r){super(n),this.code=t,this.recovery=r}},D=e=>typeof e=="string"&&/^[A-Z][A-Z0-9_]{0,63}$/.test(e)?e:void 0,Ke=e=>"'"+e.replaceAll("'",process.platform==="win32"?"''":"'\\''")+"'";function Ut(e,t,n){let r=`Run only the read-only check: wombat system doctor --project ${Ke(process.cwd())} --json. Do not reinstall the runtime.`,o=(i,u,d=r)=>new L(i,u,d);if(!g(e)||e.outputVersion!==1||!g(e.discovery)||!Array.isArray(e.discovery.instances)||!Array.isArray(e.runtimeChecks))return o("SETUP_INVALID_RESPONSE","The verification response is invalid.");let s=e.discovery;if(s.status!=="available"&&s.errorCode!=="SKILL_PARSE_ERROR"){let i=D(s.errorCode)??{missing:"PLUGIN_NOT_DISCOVERED",disabled:"PLUGIN_DISABLED",ambiguous:"PLUGIN_AMBIGUOUS"}[String(s.status)]??"PLUGIN_DISCOVERY_UNAVAILABLE",u={LOCAL_ACCESS_DENIED:"Local permissions prevented Codex plugin discovery. Run the check in a terminal allowed to access Codex.",CODEX_DISCONNECTED:"Codex disconnected during plugin discovery.",CODEX_TIMEOUT:"Codex plugin discovery timed out.",SETUP_TIMEOUT:"Plugin discovery exceeded the setup deadline.",CANCELLED:"Plugin discovery did not complete before cancellation or the setup deadline.",SKILL_PARSE_ERROR:"Codex reported Skill parsing errors in this project.",PLUGIN_NOT_DISCOVERED:"Codex did not discover the installed Wombat Skill in this project.",PLUGIN_DISABLED:"The Wombat Skill is disabled in this project.",PLUGIN_AMBIGUOUS:"More than one Wombat Skill is enabled in this project."},d=["PLUGIN_DISABLED","PLUGIN_AMBIGUOUS","PLUGIN_NOT_DISCOVERED","SKILL_PARSE_ERROR"].includes(i)?"Check this project's plugin settings and Skill errors in Codex first. ":"";return o(i,u[i]??"Codex plugin discovery is unavailable.",d+r)}let a=Array.isArray(s.instances)?s.instances.filter(i=>g(i)&&i.enabled===!0):[],c=a.length===1&&g(a[0])?a[0]:null;if(!c||c.name!==t+":wombat"||typeof c.path!="string"||!C.default.isAbsolute(c.path))return o("PLUGIN_SELECTION_MISMATCH","Codex selected a different or ambiguous Wombat Skill.","Select one intended Wombat plugin in Codex. "+r);if(s.errorCode==="SKILL_PARSE_ERROR"&&Array.isArray(s.parseErrorPaths)&&s.parseErrorPaths.includes(c.path))return o("SKILL_PARSE_ERROR","Codex could not parse the selected Wombat Skill.");let h=e.runtimeChecks.filter(i=>g(i)&&i.path===c.path);if(h.length!==1||!g(h[0])){let i=Array.isArray(e.errorCodes)&&e.errorCodes.some(u=>["SETUP_TIMEOUT","CANCELLED"].includes(String(u)));return o(i?"SETUP_TIMEOUT":"PLUGIN_RUNTIME_CHECK_MISSING",i?"The setup deadline expired before runtime compatibility could be verified.":"No unique runtime check was returned for the discovered Wombat Skill.")}let l=h[0];if(l.status!=="compatible"){let i=["modified","incompatible","unmanaged"].includes(String(l.status))?"Repair only the plugin with --plugin-only (PowerShell: -PluginOnly); keep any custom prefix. Do not reinstall the runtime.":r;return o(D(l.errorCode)??{modified:"SKILL_RESOURCE_CHANGED",incompatible:"SKILL_RUNTIME_MISMATCH",unmanaged:"PLUGIN_UNMANAGED"}[String(l.status)]??"PLUGIN_RUNTIME_UNAVAILABLE",`The discovered Wombat Skill runtime check is ${["modified","incompatible","unmanaged","unavailable"].includes(String(l.status))?l.status:"invalid"}.`,i)}if(l.pluginVersion!==n.version){let i=typeof l.pluginVersion=="string"&&/^[0-9A-Za-z][0-9A-Za-z._-]{0,63}$/.test(l.pluginVersion)?l.pluginVersion:"unknown";return o("PLUGIN_VERSION_MISMATCH",`Expected plugin ${n.version}; Codex discovered ${i}.`,"Refresh only the plugin with --plugin-only (PowerShell: -PluginOnly); keep any custom prefix. Do not reinstall the runtime.")}if(l.skillContentHash!==n.skillContentHash)return o("PLUGIN_CONTENT_MISMATCH","The discovered Skill content differs from the installed runtime bundle.","Repair only the plugin with --plugin-only (PowerShell: -PluginOnly); keep any custom prefix. Do not reinstall the runtime.")}function Mt(e){if(!g(e))return{};let t=new Set(["available","unavailable","missing","disabled","ambiguous","compatible","modified","incompatible","unmanaged"]),n=o=>typeof o=="string"&&o.length<=4096&&!/[\x00-\x1f\x7f]/.test(o)?o:null,r=g(e.discovery)?e.discovery:{};return{discovery:{status:t.has(String(r.status))?r.status:null,errorCode:D(r.errorCode)??null,instances:Array.isArray(r.instances)?r.instances.slice(0,32).filter(g).map(o=>({name:n(o.name),path:n(o.path),enabled:o.enabled===!0})):[],parseErrorPaths:Array.isArray(r.parseErrorPaths)?r.parseErrorPaths.slice(0,32).map(n).filter(Boolean):[]},runtimeVersion:n(e.runtimeVersion),runtimeChecks:Array.isArray(e.runtimeChecks)?e.runtimeChecks.slice(0,32).filter(g).map(o=>({path:n(o.path),status:t.has(String(o.status))?o.status:null,pluginVersion:n(o.pluginVersion),skillContentHash:typeof o.skillContentHash=="string"&&/^[a-f0-9]{64}$/.test(o.skillContentHash)?o.skillContentHash:null,missingCapabilities:Array.isArray(o.missingCapabilities)?o.missingCapabilities.slice(0,32).map(n).filter(Boolean):[],errorCode:D(o.errorCode)??null})):[],errorCodes:Array.isArray(e.errorCodes)?e.errorCodes.slice(0,32).map(D).filter(Boolean):[]}}function Ft(e){let t=C.default.join(e,".agents/plugins/marketplace.json"),n=(0,S.lstatSync)(t);if(!n.isFile()||n.isSymbolicLink()||n.size>65536)throw new Error("Invalid bundled plugin marketplace.");let r=JSON.parse((0,S.readFileSync)(t,"utf8"));if(!g(r)||r.name!=="wombat-local"||!Array.isArray(r.plugins))throw new Error("Invalid bundled plugin marketplace.");let o=r.plugins;if(o.length!==1||!["wombat"].every(s=>o.some(a=>g(a)&&a.name===s&&g(a.source)&&a.source.source==="local"&&a.source.path==="./plugin")))throw new Error("Invalid bundled plugin marketplace.")}async function Xe(e,t={}){let n=C.default.resolve(e);Ft(n);let r=t.env??process.env,o=t.binary??r.WOMBAT_CODEX_BIN??"codex",s=/\.[cm]?js$/.test(o)?[process.execPath,o]:[o],a=(0,S.mkdtempSync)(C.default.join(Je.default.tmpdir(),"wombat-plugin-install-")),c=new AbortController,h=()=>c.abort(),l=t.signal?AbortSignal.any([t.signal,c.signal]):c.signal,i="list",u=!1,d={};process.once("SIGINT",h),process.once("SIGTERM",h);try{let m=async(y,G,Ze=s,Ye=[0])=>{i=y;let ee=C.default.join(a,i+".log"),f=await ze({command:[...Ze,...G],cwd:process.cwd(),env:{...r,WOMBAT_CODEX_BIN:o},logFile:ee,timeoutMs:t.timeoutMs??2e4,maxBytes:1024*1024,signal:l});if(f.exitCode===null||!Ye.includes(f.exitCode)||f.timedOut||f.outputLimit||f.interrupted||f.closeTimedOut||f.spawnError||f.logError){let Qe=f.spawnError?"executable unavailable":f.timedOut?"timeout":f.outputLimit?"output limit":f.interrupted?"cancelled":f.closeTimedOut?"cleanup incomplete":f.logError?"log write failed":"exit "+f.exitCode,et=f.spawnError?("code"in f.spawnError?D(f.spawnError.code):void 0)??"EXECUTABLE_UNAVAILABLE":f.timedOut?"TIMEOUT":f.outputLimit?"OUTPUT_LIMIT":f.interrupted?"CANCELLED":f.closeTimedOut?"CLEANUP_INCOMPLETE":f.logError?"LOG_WRITE_FAILED":"COMMAND_FAILED",tt=i==="verify"?`Run only the read-only check: wombat system doctor --project ${Ke(process.cwd())} --json. Do not reinstall the runtime.`:"Retry only the plugin stage with --plugin-only (PowerShell: -PluginOnly); keep any custom prefix. Do not reinstall the runtime.";throw new L(et,`Codex plugin ${i} failed (${Qe}).`,tt)}return He((0,S.readFileSync)(ee,"utf8"))},w=JSON.parse(await m("list",["plugin","list","--marketplace","wombat-local","--json"]));if(!g(w)||!Array.isArray(w.installed))throw new Error("Unrecognized Codex plugin list. Wombat is installed.");if(w.installed.filter(y=>g(y)&&y.enabled===!0&&typeof y.pluginId=="string"&&y.pluginId==="wombat@wombat-local").length>1)throw new Error("Multiple Wombat plugins are enabled. Choose one in Codex, then rerun the installer.");let v="wombat";i="requirements";let A=C.default.join(n,"plugin","wombat-runtime.json"),P=(0,S.lstatSync)(A);if(!P.isFile()||P.isSymbolicLink()||P.size>65536)throw new Error("Invalid bundled runtime requirements.");let k=JSON.parse((0,S.readFileSync)(A,"utf8"));if(!g(k)||k.format!==1||typeof k.version!="string"||!/^[0-9A-Za-z][0-9A-Za-z._-]{0,63}$/.test(k.version)||typeof k.skillContentHash!="string"||!/^[a-f0-9]{64}$/.test(k.skillContentHash))throw new Error("Invalid bundled runtime requirements.");d={plugin:v,expected:{pluginVersion:k.version,skillContentHash:k.skillContentHash}};let T=JSON.parse(await m("catalogs",["plugin","marketplace","list","--json"]));if(!g(T)||!Array.isArray(T.marketplaces))throw new Error("Unrecognized Codex marketplace list. Wombat is installed.");let I=T.marketplaces.find(y=>g(y)&&y.name==="wombat-local"),_;if(I!==void 0){if(!g(I)||!g(I.marketplaceSource)||I.marketplaceSource.sourceType!=="local"||typeof I.marketplaceSource.source!="string")throw new Error("Existing wombat-local marketplace is not local. Choose its source in Codex before retrying.");C.default.resolve(I.marketplaceSource.source)!==n&&(_=I.marketplaceSource.source,await m("unregister",["plugin","marketplace","remove","wombat-local"]))}let p=!1;try{await m("marketplace",["plugin","marketplace","add",n]),p=!0;let y=JSON.parse(await m("install",["plugin","add",v+"@wombat-local","--json"]));if(!g(y)||y.pluginId!==v+"@wombat-local"||typeof y.installedPath!="string"||!C.default.isAbsolute(y.installedPath))throw new Error("Unrecognized Codex plugin installation result. Check the plugin in Codex.")}catch(y){let G=i;if(_&&!l.aborted)try{p&&await m("restore-remove",["plugin","marketplace","remove","wombat-local"]),await m("restore",["plugin","marketplace","add",_])}catch{throw new Error("Plugin installation and marketplace recovery failed. Wombat is installed; restore wombat-local in Codex and rerun the installer.")}throw i=G,y}let b,R=await m("verify",["system","doctor","--project",process.cwd(),"--json"],[process.execPath,C.default.resolve(n,"../wombat.js")],[0,2]);try{b=JSON.parse(R)}catch{throw new L("SETUP_INVALID_RESPONSE","The verification response is not valid JSON.","Run wombat system doctor --json to check the installed runtime. Do not reinstall the runtime.")}d={...d,...Mt(b)};let N=Ut(b,v,k);if(N)throw N;u=!0;let U=g(b)&&g(b.discovery)?b.discovery:{};return{name:v,discoveryVerified:U.status==="available",warningPaths:Array.isArray(U.parseErrorPaths)?U.parseErrorPaths.filter(y=>typeof y=="string"&&y.length<=4096&&!/[\x00-\x1f\x7f]/.test(y)).slice(0,32):[]}}catch(m){for(let P of(0,S.readdirSync)(a))(0,S.rmSync)(C.default.join(a,P),{force:!0});let w=m instanceof L?m:new L("PLUGIN_INSTALL_FAILED",m instanceof SyntaxError?"Codex returned invalid JSON.":m instanceof Error?m.message:"Plugin installation failed.","Check this stage before retrying only the plugin installation with --plugin-only (PowerShell: -PluginOnly)."),E=C.default.join(a,"diagnostic.json"),v={outputVersion:1,stage:i,code:w.code,project:process.cwd(),checkedAt:new Date().toISOString(),...d},A=!1;try{(0,S.writeFileSync)(E,JSON.stringify(v,null,2)+`
`,{mode:384}),A=!0}catch{}throw new Error(`Wombat is installed. Plugin ${i} failed [${w.code}]: ${w.message} ${w.recovery} Installed files and data are retained.${A?" Diagnostic summary: "+E:" Diagnostic summary could not be saved."}`)}finally{process.off("SIGINT",h),process.off("SIGTERM",h),u&&(0,S.rmSync)(a,{recursive:!0,force:!0})}}if(process.argv[1]==="-"||C.default.basename(process.argv[1]??"")==="install-plugin-bootstrap.ts"){let e=process.argv[2];!e||process.argv.length!==3?(console.error("A bundled plugin marketplace path is required."),process.exitCode=2):Xe(e).then(t=>{t.discoveryVerified?console.log(`Installed and verified ${t.name} through Codex for ${process.cwd()}. Start a new Codex project conversation and ask $${t.name}:wombat about your usage. Hook trust is optional for log queries.`):console.log(`Wombat runtime and plugin files are installed; the discovered Wombat bundle passed compatibility checks. Codex project discovery remains incomplete [SKILL_PARSE_ERROR].${t.warningPaths.length?" Check: "+t.warningPaths.join(", "):" Check Skill parsing errors in Codex."} Fix the reported Skill errors, then run wombat system doctor --json. Do not reinstall Wombat.`)},t=>{console.error(t instanceof Error?t.message:"Plugin installation failed. Wombat is installed."),process.exitCode=1})}0&&(module.exports={installBundledPlugin});
// END generated Wombat plugin bootstrap
WOMBAT_PLUGIN_BOOTSTRAP
}
install_plugin
