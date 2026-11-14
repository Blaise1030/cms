const __ORSO_CHECKS_CSS__ = "@layer properties{@supports (((-webkit-hyphens:none)) and (not (margin-trim:block))) or ((-moz-orient:inline) and (not (color:rgb(from red r g b)))){*,:before,:after,::backdrop{--tw-border-style:solid;--tw-leading:initial;--tw-font-weight:initial;--tw-tracking:initial;--tw-ordinal:initial;--tw-slashed-zero:initial;--tw-numeric-figure:initial;--tw-numeric-spacing:initial;--tw-numeric-fraction:initial;--tw-shadow:0 0 #0000;--tw-shadow-color:initial;--tw-shadow-alpha:100%;--tw-inset-shadow:0 0 #0000;--tw-inset-shadow-color:initial;--tw-inset-shadow-alpha:100%;--tw-ring-color:initial;--tw-ring-shadow:0 0 #0000;--tw-inset-ring-color:initial;--tw-inset-ring-shadow:0 0 #0000;--tw-ring-inset:initial;--tw-ring-offset-width:0px;--tw-ring-offset-color:#fff;--tw-ring-offset-shadow:0 0 #0000;--tw-outline-style:solid;--tw-translate-x:0;--tw-translate-y:0;--tw-translate-z:0}}}@layer theme{:root,:host{--font-sans:-apple-system, BlinkMacSystemFont, \"Segoe UI\", Roboto, \"Helvetica Neue\", \"Noto Sans\", Arial, sans-serif, \"Apple Color Emoji\", \"Segoe UI Emoji\", \"Segoe UI Symbol\", \"Noto Color Emoji\";--font-mono:ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, \"Liberation Mono\", \"Courier New\", monospace;--color-emerald-500:oklch(69.6% .17 162.48);--color-white:#fff;--spacing:.25rem;--text-xs:.75rem;--text-xs--line-height:calc(1 / .75);--text-sm:.875rem;--text-sm--line-height:calc(1.25 / .875);--font-weight-medium:500;--tracking-tight:-.025em;--tracking-wide:.025em;--leading-snug:1.375;--radius-xl:.75rem;--default-transition-duration:.15s;--default-transition-timing-function:cubic-bezier(.4, 0, .2, 1);--default-font-family:var(--font-sans);--default-mono-font-family:var(--font-mono)}}@layer base{*,:after,:before,::backdrop{box-sizing:border-box;border:0 solid;margin:0;padding:0}::file-selector-button{box-sizing:border-box;border:0 solid;margin:0;padding:0}html,:host{-webkit-text-size-adjust:100%;tab-size:4;line-height:1.5;font-family:var(--default-font-family,-apple-system, BlinkMacSystemFont, \"Segoe UI\", Roboto, \"Helvetica Neue\", \"Noto Sans\", Arial, sans-serif, \"Apple Color Emoji\", \"Segoe UI Emoji\", \"Segoe UI Symbol\", \"Noto Color Emoji\");font-feature-settings:var(--default-font-feature-settings,normal);font-variation-settings:var(--default-font-variation-settings,normal);-webkit-tap-highlight-color:transparent}hr{height:0;color:inherit;border-top-width:1px}abbr:where([title]){-webkit-text-decoration:underline dotted;text-decoration:underline dotted}h1,h2,h3,h4,h5,h6{font-size:inherit;font-weight:inherit}a{color:inherit;-webkit-text-decoration:inherit;text-decoration:inherit}b,strong{font-weight:bolder}code,kbd,samp,pre{font-family:var(--default-mono-font-family,ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, \"Liberation Mono\", \"Courier New\", monospace);font-feature-settings:var(--default-mono-font-feature-settings,normal);font-variation-settings:var(--default-mono-font-variation-settings,normal);font-size:1em}small{font-size:80%}sub,sup{vertical-align:baseline;font-size:75%;line-height:0;position:relative}sub{bottom:-.25em}sup{top:-.5em}table{text-indent:0;border-color:inherit;border-collapse:collapse}:-moz-focusring:where(:not(iframe)){outline:auto}progress{vertical-align:baseline}summary{display:list-item}ol,ul,menu{list-style:none}img,svg,video,canvas,audio,iframe,embed,object{vertical-align:middle;display:block}img,video{max-width:100%;height:auto}button,input,select,optgroup,textarea{font:inherit;font-feature-settings:inherit;font-variation-settings:inherit;letter-spacing:inherit;color:inherit;opacity:1;background-color:#0000;border-radius:0}::file-selector-button{font:inherit;font-feature-settings:inherit;font-variation-settings:inherit;letter-spacing:inherit;color:inherit;opacity:1;background-color:#0000;border-radius:0}:where(select:is([multiple],[size])) optgroup{font-weight:bolder}:where(select:is([multiple],[size])) optgroup option{padding-inline-start:20px}::file-selector-button{margin-inline-end:4px}::placeholder{opacity:1}@supports (not ((-webkit-appearance:-apple-pay-button))) or (contain-intrinsic-size:1px){::placeholder{color:currentColor}@supports (color:color-mix(in lab,red,red)){::placeholder{color:color-mix(in oklab,currentcolor 50%,transparent)}}}textarea{resize:vertical}::-webkit-search-decoration{-webkit-appearance:none}::-webkit-date-and-time-value{min-height:1lh;text-align:inherit}::-webkit-datetime-edit{display:inline-flex}::-webkit-datetime-edit-fields-wrapper{padding:0}::-webkit-datetime-edit{padding-block:0}::-webkit-datetime-edit-year-field{padding-block:0}::-webkit-datetime-edit-month-field{padding-block:0}::-webkit-datetime-edit-day-field{padding-block:0}::-webkit-datetime-edit-hour-field{padding-block:0}::-webkit-datetime-edit-minute-field{padding-block:0}::-webkit-datetime-edit-second-field{padding-block:0}::-webkit-datetime-edit-millisecond-field{padding-block:0}::-webkit-datetime-edit-meridiem-field{padding-block:0}::-webkit-calendar-picker-indicator{line-height:1}:-moz-ui-invalid{box-shadow:none}button,input:where([type=button],[type=reset],[type=submit]){appearance:button}::file-selector-button{appearance:button}::-webkit-inner-spin-button{height:auto}::-webkit-outer-spin-button{height:auto}[hidden]:where(:not([hidden=until-found])){display:none!important}}@layer components;@layer utilities{.pointer-events-auto{pointer-events:auto}.pointer-events-none{pointer-events:none}.fixed{position:fixed}.m-0{margin:0}.mt-0\\.5{margin-top:calc(var(--spacing) * .5)}.mt-1{margin-top:var(--spacing)}.mt-2{margin-top:calc(var(--spacing) * 2)}.flex{display:flex}.grid{display:grid}.inline-flex{display:inline-flex}.size-3{width:calc(var(--spacing) * 3);height:calc(var(--spacing) * 3)}.size-3\\.5{width:calc(var(--spacing) * 3.5);height:calc(var(--spacing) * 3.5)}.size-7{width:calc(var(--spacing) * 7);height:calc(var(--spacing) * 7)}.h-6{height:calc(var(--spacing) * 6)}.h-8{height:calc(var(--spacing) * 8)}.h-9{height:calc(var(--spacing) * 9)}.h-10{height:calc(var(--spacing) * 10)}.max-h-\\[min\\(32rem\\,calc\\(100vh-80px\\)\\)\\]{max-height:min(32rem,100vh - 80px)}.min-h-0{min-height:0}.w-\\[min\\(22rem\\,calc\\(100vw-24px\\)\\)\\]{width:min(22rem,100vw - 24px)}.w-fit{width:fit-content}.min-w-0{min-width:0}.flex-1{flex:1}.shrink-0{flex-shrink:0}.list-none{list-style-type:none}.flex-col{flex-direction:column}.items-baseline{align-items:baseline}.items-center{align-items:center}.items-end{align-items:flex-end}.items-start{align-items:flex-start}.justify-between{justify-content:space-between}.justify-center{justify-content:center}.gap-0{gap:0}.gap-0\\.5{gap:calc(var(--spacing) * .5)}.gap-1{gap:var(--spacing)}.gap-1\\.5{gap:calc(var(--spacing) * 1.5)}.gap-2{gap:calc(var(--spacing) * 2)}.gap-2\\.5{gap:calc(var(--spacing) * 2.5)}.gap-3{gap:calc(var(--spacing) * 3)}.gap-6{gap:calc(var(--spacing) * 6)}.overflow-auto{overflow:auto}.overflow-hidden{overflow:hidden}.rounded-full{border-radius:2147483647px}.rounded-lg{border-radius:var(--radius)}.rounded-md{border-radius:calc(var(--radius) - 2px)}.rounded-xl{border-radius:var(--radius-xl)}.border{border-style:var(--tw-border-style);border-width:1px}.border-b{border-bottom-style:var(--tw-border-style);border-bottom-width:1px}.border-\\[color-mix\\(in_oklch\\,var\\(--warning\\)_35\\%\\,transparent\\)\\]{border-color:var(--warning)}@supports (color:color-mix(in lab,red,red)){.border-\\[color-mix\\(in_oklch\\,var\\(--warning\\)_35\\%\\,transparent\\)\\]{border-color:color-mix(in oklch,var(--warning) 35%,transparent)}}.border-border{border-color:var(--border)}.border-destructive\\/30{border-color:var(--destructive)}@supports (color:color-mix(in lab,red,red)){.border-destructive\\/30{border-color:color-mix(in oklab,var(--destructive) 30%,transparent)}}.border-transparent{border-color:#0000}.bg-\\[color-mix\\(in_oklch\\,var\\(--warning\\)_18\\%\\,transparent\\)\\]{background-color:var(--warning)}@supports (color:color-mix(in lab,red,red)){.bg-\\[color-mix\\(in_oklch\\,var\\(--warning\\)_18\\%\\,transparent\\)\\]{background-color:color-mix(in oklch,var(--warning) 18%,transparent)}}.bg-background{background-color:var(--background)}.bg-card{background-color:var(--card)}.bg-destructive,.bg-destructive\\/15{background-color:var(--destructive)}@supports (color:color-mix(in lab,red,red)){.bg-destructive\\/15{background-color:color-mix(in oklab,var(--destructive) 15%,transparent)}}.bg-primary{background-color:var(--primary)}.bg-secondary{background-color:var(--secondary)}.btn-raised{background-image:linear-gradient(#fff,#0000 55%,#0000000f)}@supports (color:color-mix(in lab,red,red)){.btn-raised{background-image:linear-gradient(to bottom,color-mix(in oklch,white var(--raised-gloss),transparent),transparent 55%,oklch(0% none none/.06))}}.btn-raised{--tw-inset-shadow:inset 0 1px 0 white, inset 0 -1px 0 #0000001a}@supports (color:color-mix(in lab,red,red)){.btn-raised{--tw-inset-shadow:inset 0 1px 0 color-mix(in oklch, white calc(var(--raised-gloss) * 1.4), transparent), inset 0 -1px 0 oklch(0% none none/.1)}}.btn-raised{--tw-shadow:0 1px 1px var(--raised-tint,black), 0 2px 6px -2px var(--raised-tint,black)}@supports (color:color-mix(in lab,red,red)){.btn-raised{--tw-shadow:0 1px 1px color-mix(in oklch, var(--raised-tint,black) 20%, transparent), 0 2px 6px -2px color-mix(in oklch, var(--raised-tint,black) 25%, transparent)}}.btn-raised{box-shadow:var(--tw-inset-shadow),var(--tw-inset-ring-shadow,0 0 #0000),var(--tw-ring-offset-shadow,0 0 #0000),var(--tw-ring-shadow,0 0 #0000),var(--tw-shadow)}.btn-raised-soft{background-image:linear-gradient(#fff,#0000)}@supports (color:color-mix(in lab,red,red)){.btn-raised-soft{background-image:linear-gradient(to bottom,color-mix(in oklch,white var(--raised-soft-gloss),transparent),transparent)}}.btn-raised-soft{--tw-inset-shadow:inset 0 1px 0 white}@supports (color:color-mix(in lab,red,red)){.btn-raised-soft{--tw-inset-shadow:inset 0 1px 0 color-mix(in oklch, white var(--raised-soft-gloss), transparent)}}.btn-raised-soft{--tw-shadow:0 0 0 1px var(--foreground), 0 1px 1px var(--foreground), 0 2px 4px -2px var(--foreground)}@supports (color:color-mix(in lab,red,red)){.btn-raised-soft{--tw-shadow:0 0 0 1px color-mix(in oklch, var(--foreground) 7%, transparent), 0 1px 1px color-mix(in oklch, var(--foreground) 5%, transparent), 0 2px 4px -2px color-mix(in oklch, var(--foreground) 6%, transparent)}}.btn-raised-soft{box-shadow:var(--tw-inset-shadow),var(--tw-inset-ring-shadow,0 0 #0000),var(--tw-ring-offset-shadow,0 0 #0000),var(--tw-ring-shadow,0 0 #0000),var(--tw-shadow)}.px-0\\.5{padding-inline:calc(var(--spacing) * .5)}.px-2{padding-inline:calc(var(--spacing) * 2)}.px-3{padding-inline:calc(var(--spacing) * 3)}.px-4{padding-inline:calc(var(--spacing) * 4)}.px-6{padding-inline:calc(var(--spacing) * 6)}.py-0{padding-block:0}.py-0\\.5{padding-block:calc(var(--spacing) * .5)}.py-1{padding-block:var(--spacing)}.py-2{padding-block:calc(var(--spacing) * 2)}.py-2\\.5{padding-block:calc(var(--spacing) * 2.5)}.py-3{padding-block:calc(var(--spacing) * 3)}.py-6{padding-block:calc(var(--spacing) * 6)}.pb-0\\.5{padding-bottom:calc(var(--spacing) * .5)}.pb-3{padding-bottom:calc(var(--spacing) * 3)}.text-sm{font-size:var(--text-sm);line-height:var(--tw-leading,var(--text-sm--line-height))}.text-xs{font-size:var(--text-xs);line-height:var(--tw-leading,var(--text-xs--line-height))}.text-\\[0\\.68rem\\]{font-size:.68rem}.text-\\[0\\.72rem\\]{font-size:.72rem}.leading-snug{--tw-leading:var(--leading-snug);line-height:var(--leading-snug)}.font-medium{--tw-font-weight:var(--font-weight-medium);font-weight:var(--font-weight-medium)}.tracking-tight{--tw-tracking:var(--tracking-tight);letter-spacing:var(--tracking-tight)}.tracking-wide{--tw-tracking:var(--tracking-wide);letter-spacing:var(--tracking-wide)}.break-words{overflow-wrap:break-word}.whitespace-nowrap{white-space:nowrap}.text-\\[color-mix\\(in_oklch\\,var\\(--warning\\)_90\\%\\,white\\)\\]{color:var(--warning)}@supports (color:color-mix(in lab,red,red)){.text-\\[color-mix\\(in_oklch\\,var\\(--warning\\)_90\\%\\,white\\)\\]{color:color-mix(in oklch,var(--warning) 90%,white)}}.text-card-foreground{color:var(--card-foreground)}.text-destructive{color:var(--destructive)}.text-emerald-500{color:var(--color-emerald-500)}.text-foreground{color:var(--foreground)}.text-muted-foreground{color:var(--muted-foreground)}.text-primary{color:var(--primary)}.text-primary-foreground{color:var(--primary-foreground)}.text-secondary-foreground{color:var(--secondary-foreground)}.text-white{color:var(--color-white)}.uppercase{text-transform:uppercase}.tabular-nums{--tw-numeric-spacing:tabular-nums;font-variant-numeric:var(--tw-ordinal, ) var(--tw-slashed-zero, ) var(--tw-numeric-figure, ) var(--tw-numeric-spacing, ) var(--tw-numeric-fraction, )}.underline-offset-4{text-underline-offset:4px}.opacity-35{opacity:.35}.shadow{--tw-shadow:0 1px 3px 0 var(--tw-shadow-color,#0000001a), 0 1px 2px -1px var(--tw-shadow-color,#0000001a);box-shadow:var(--tw-inset-shadow),var(--tw-inset-ring-shadow),var(--tw-ring-offset-shadow),var(--tw-ring-shadow),var(--tw-shadow)}.shadow-lg{--tw-shadow:0 10px 15px -3px var(--tw-shadow-color,#0000001a), 0 4px 6px -4px var(--tw-shadow-color,#0000001a);box-shadow:var(--tw-inset-shadow),var(--tw-inset-ring-shadow),var(--tw-ring-offset-shadow),var(--tw-ring-shadow),var(--tw-shadow)}.shadow-sm{--tw-shadow:0 1px 3px 0 var(--tw-shadow-color,#0000001a), 0 1px 2px -1px var(--tw-shadow-color,#0000001a);box-shadow:var(--tw-inset-shadow),var(--tw-inset-ring-shadow),var(--tw-ring-offset-shadow),var(--tw-ring-shadow),var(--tw-shadow)}.outline{outline-style:var(--tw-outline-style);outline-width:1px}.transition-all{transition-property:all;transition-timing-function:var(--tw-ease,var(--default-transition-timing-function));transition-duration:var(--tw-duration,var(--default-transition-duration))}.outline-none{--tw-outline-style:none;outline-style:none}.\\[--raised-tint\\:var\\(--destructive\\)\\]{--raised-tint:var(--destructive)}.\\[--raised-tint\\:var\\(--primary\\)\\]{--raised-tint:var(--primary)}@media(hover:hover){.hover\\:bg-accent:hover{background-color:var(--accent)}.hover\\:bg-background:hover{background-color:var(--background)}.hover\\:bg-destructive\\/90:hover{background-color:var(--destructive)}@supports (color:color-mix(in lab,red,red)){.hover\\:bg-destructive\\/90:hover{background-color:color-mix(in oklab,var(--destructive) 90%,transparent)}}.hover\\:bg-primary\\/90:hover{background-color:var(--primary)}@supports (color:color-mix(in lab,red,red)){.hover\\:bg-primary\\/90:hover{background-color:color-mix(in oklab,var(--primary) 90%,transparent)}}.hover\\:bg-secondary\\/80:hover{background-color:var(--secondary)}@supports (color:color-mix(in lab,red,red)){.hover\\:bg-secondary\\/80:hover{background-color:color-mix(in oklab,var(--secondary) 80%,transparent)}}.hover\\:text-accent-foreground:hover{color:var(--accent-foreground)}.hover\\:underline:hover{text-decoration-line:underline}}.focus-visible\\:border-ring:focus-visible{border-color:var(--ring)}.focus-visible\\:ring-3:focus-visible{--tw-ring-shadow:var(--tw-ring-inset, ) 0 0 0 calc(3px + var(--tw-ring-offset-width)) var(--tw-ring-color,currentcolor);box-shadow:var(--tw-inset-shadow),var(--tw-inset-ring-shadow),var(--tw-ring-offset-shadow),var(--tw-ring-shadow),var(--tw-shadow)}.focus-visible\\:ring-ring\\/50:focus-visible{--tw-ring-color:var(--ring)}@supports (color:color-mix(in lab,red,red)){.focus-visible\\:ring-ring\\/50:focus-visible{--tw-ring-color:color-mix(in oklab, var(--ring) 50%, transparent)}}.active\\:translate-y-0:active{--tw-translate-y:0px;translate:var(--tw-translate-x) var(--tw-translate-y)}.active\\:translate-y-px:active{--tw-translate-y:1px;translate:var(--tw-translate-x) var(--tw-translate-y)}.disabled\\:pointer-events-none:disabled{pointer-events:none}.disabled\\:opacity-50:disabled{opacity:.5}@media(prefers-color-scheme:dark){.dark\\:bg-input\\/30{background-color:var(--input)}@supports (color:color-mix(in lab,red,red)){.dark\\:bg-input\\/30{background-color:color-mix(in oklab,var(--input) 30%,transparent)}}}}:host{color-scheme:dark;color:var(--foreground);--background:oklch(18% 0 0);--foreground:oklch(98.5% 0 0);--card:oklch(20.5% 0 0);--card-foreground:oklch(98.5% 0 0);--popover:oklch(20.5% 0 0);--popover-foreground:oklch(98.5% 0 0);--primary:oklch(62% .19 260);--primary-foreground:oklch(98.5% 0 0);--secondary:oklch(26.9% 0 0);--secondary-foreground:oklch(98.5% 0 0);--muted:oklch(26.9% 0 0);--muted-foreground:oklch(70.8% 0 0);--accent:oklch(26.9% 0 0);--accent-foreground:oklch(98.5% 0 0);--destructive:oklch(70.4% .191 22.216);--warning:oklch(80% .14 75);--border:oklch(100% 0 0/.12);--input:oklch(100% 0 0/.15);--ring:oklch(60% .17 260);--radius:.625rem;--raised-gloss:14%;--raised-soft-gloss:6%;font-family:Geist Variable,ui-sans-serif,system-ui,sans-serif;font-size:.8125rem;line-height:1.35}.orso-checks-root{color:var(--foreground)}@property --tw-border-style{syntax:\"*\";inherits:false;initial-value:solid}@property --tw-leading{syntax:\"*\";inherits:false}@property --tw-font-weight{syntax:\"*\";inherits:false}@property --tw-tracking{syntax:\"*\";inherits:false}@property --tw-ordinal{syntax:\"*\";inherits:false}@property --tw-slashed-zero{syntax:\"*\";inherits:false}@property --tw-numeric-figure{syntax:\"*\";inherits:false}@property --tw-numeric-spacing{syntax:\"*\";inherits:false}@property --tw-numeric-fraction{syntax:\"*\";inherits:false}@property --tw-shadow{syntax:\"*\";inherits:false;initial-value:0 0 #0000}@property --tw-shadow-color{syntax:\"*\";inherits:false}@property --tw-shadow-alpha{syntax:\"<percentage>\";inherits:false;initial-value:100%}@property --tw-inset-shadow{syntax:\"*\";inherits:false;initial-value:0 0 #0000}@property --tw-inset-shadow-color{syntax:\"*\";inherits:false}@property --tw-inset-shadow-alpha{syntax:\"<percentage>\";inherits:false;initial-value:100%}@property --tw-ring-color{syntax:\"*\";inherits:false}@property --tw-ring-shadow{syntax:\"*\";inherits:false;initial-value:0 0 #0000}@property --tw-inset-ring-color{syntax:\"*\";inherits:false}@property --tw-inset-ring-shadow{syntax:\"*\";inherits:false;initial-value:0 0 #0000}@property --tw-ring-inset{syntax:\"*\";inherits:false}@property --tw-ring-offset-width{syntax:\"<length>\";inherits:false;initial-value:0}@property --tw-ring-offset-color{syntax:\"*\";inherits:false;initial-value:#fff}@property --tw-ring-offset-shadow{syntax:\"*\";inherits:false;initial-value:0 0 #0000}@property --tw-outline-style{syntax:\"*\";inherits:false;initial-value:solid}@property --tw-translate-x{syntax:\"*\";inherits:false;initial-value:0}@property --tw-translate-y{syntax:\"*\";inherits:false;initial-value:0}@property --tw-translate-z{syntax:\"*\";inherits:false;initial-value:0}\n";
// @__NO_SIDE_EFFECTS__
function Es(e) {
  const t = /* @__PURE__ */ Object.create(null);
  for (const n of e.split(",")) t[n] = 1;
  return (n) => n in t;
}
const ne = {}, Et = [], Xe = () => {
}, Br = () => !1, Nn = (e) => e.charCodeAt(0) === 111 && e.charCodeAt(1) === 110 && // uppercase letter
(e.charCodeAt(2) > 122 || e.charCodeAt(2) < 97), Fn = (e) => e.startsWith("onUpdate:"), he = Object.assign, Ps = (e, t) => {
  const n = e.indexOf(t);
  n > -1 && e.splice(n, 1);
}, xi = Object.prototype.hasOwnProperty, J = (e, t) => xi.call(e, t), F = Array.isArray, bt = (e) => gn(e) === "[object Map]", An = (e) => gn(e) === "[object Set]", Xs = (e) => gn(e) === "[object Date]", H = (e) => typeof e == "function", ce = (e) => typeof e == "string", De = (e) => typeof e == "symbol", Q = (e) => e !== null && typeof e == "object", Vr = (e) => (Q(e) || H(e)) && H(e.then) && H(e.catch), Ur = Object.prototype.toString, gn = (e) => Ur.call(e), vi = (e) => gn(e).slice(8, -1), Kr = (e) => gn(e) === "[object Object]", Os = (e) => ce(e) && e !== "NaN" && e[0] !== "-" && "" + parseInt(e, 10) === e, Qt = /* @__PURE__ */ Es(
  // the leading comma is intentional so empty string "" is also included
  ",key,ref,ref_for,ref_key,onVnodeBeforeMount,onVnodeMounted,onVnodeBeforeUpdate,onVnodeUpdated,onVnodeBeforeUnmount,onVnodeUnmounted"
), zn = (e) => {
  const t = /* @__PURE__ */ Object.create(null);
  return ((n) => t[n] || (t[n] = e(n)));
}, _i = /-\w/g, Fe = /* @__PURE__ */ zn((e) => e.replace(_i, (t) => t.slice(1).toUpperCase())), wi = /\B([A-Z])/g, Ot = /* @__PURE__ */ zn(
  (e) => e.replace(wi, "-$1").toLowerCase()
), Gr = /* @__PURE__ */ zn((e) => e.charAt(0).toUpperCase() + e.slice(1)), Jn = /* @__PURE__ */ zn((e) => e ? `on${Gr(e)}` : ""), Ye = (e, t) => !Object.is(e, t), Yn = (e, ...t) => {
  for (let n = 0; n < e.length; n++)
    e[n](...t);
}, qr = (e, t, n, s = !1) => {
  Object.defineProperty(e, t, {
    configurable: !0,
    enumerable: !1,
    writable: s,
    value: n
  });
}, ki = (e) => {
  const t = parseFloat(e);
  return isNaN(t) ? e : t;
};
let Qs;
const Dn = () => Qs || (Qs = typeof globalThis < "u" ? globalThis : typeof self < "u" ? self : typeof window < "u" ? window : typeof global < "u" ? global : {});
function Is(e) {
  if (F(e)) {
    const t = {};
    for (let n = 0; n < e.length; n++) {
      const s = e[n], r = ce(s) ? Ti(s) : Is(s);
      if (r)
        for (const o in r)
          t[o] = r[o];
    }
    return t;
  } else if (ce(e) || Q(e))
    return e;
}
const Si = /;(?![^(]*\))/g, Ci = /:([^]+)/, Ai = /"(?:[^"\\]|\\[^])*"|'(?:[^'\\]|\\[^])*'|\\[^]|\/\*[^]*?\*\//g;
function Ti(e) {
  const t = {};
  return e.replace(Ai, (n) => n.startsWith("/*") ? "" : n).split(Si).forEach((n) => {
    if (n) {
      const s = n.split(Ci);
      s.length > 1 && (t[s[0].trim()] = s[1].trim());
    }
  }), t;
}
function yt(e) {
  let t = "";
  if (ce(e))
    t = e;
  else if (F(e))
    for (let n = 0; n < e.length; n++) {
      const s = yt(e[n]);
      s && (t += s + " ");
    }
  else if (Q(e))
    for (const n in e)
      e[n] && (t += n + " ");
  return t.trim();
}
const Ei = "itemscope,allowfullscreen,disablepictureinpicture,disableremoteplayback,formnovalidate,ismap,nomodule,novalidate,playsinline,readonly", Pi = /* @__PURE__ */ Es(Ei);
function Jr(e) {
  return !!e || e === "";
}
function Oi(e, t, n) {
  if (e.length !== t.length) return !1;
  let s = !0;
  for (let r = 0; s && r < e.length; r++)
    s = Hn(e[r], t[r], n);
  return s;
}
function Zs(e, t, n) {
  if (e.size !== t.size) return !1;
  const s = Array.from(t), r = new Uint8Array(s.length);
  for (const o of e) {
    let i = -1;
    for (let l = 0; l < s.length; l++)
      if (!r[l] && Hn(o, s[l], n)) {
        i = l;
        break;
      }
    if (i < 0) return !1;
    r[i] = 1;
  }
  return !0;
}
function Ii(e, t, n) {
  let s = bt(e), r = bt(t);
  if (s || r || (s = An(e), r = An(t), s || r))
    return s && r ? Zs(e, t, n) : !1;
  const o = Object.keys(e).length, i = Object.keys(t).length;
  if (o !== i)
    return !1;
  for (const l in e) {
    const c = e.hasOwnProperty(l), u = t.hasOwnProperty(l);
    if (c && !u || !c && u || !Hn(e[l], t[l], n))
      return !1;
  }
  return String(e) === String(t);
}
function er(e, t, n, s) {
  n || (n = [/* @__PURE__ */ new Map(), /* @__PURE__ */ new Map()]);
  const [r, o] = n;
  if (r.has(e) || o.has(t))
    return r.get(e) === t && o.get(t) === e;
  r.set(e, t), o.set(t, e);
  const i = s(e, t, n);
  return r.delete(e), o.delete(t), i;
}
function Hn(e, t, n) {
  if (e === t) return !0;
  let s = Xs(e), r = Xs(t);
  return s || r ? s && r ? e.getTime() === t.getTime() : !1 : (s = De(e), r = De(t), s || r ? e === t : (s = F(e), r = F(t), s || r ? s && r ? er(e, t, n, Oi) : !1 : (s = Q(e), r = Q(t), s || r ? !s || !r ? !1 : er(e, t, n, Ii) : String(e) === String(t))));
}
const Yr = (e) => !!(e && e.__v_isRef === !0), lt = (e) => ce(e) ? e : e == null ? "" : F(e) || Q(e) && (e.toString === Ur || !H(e.toString)) ? Yr(e) ? lt(e.value) : JSON.stringify(e, Xr, 2) : String(e), Xr = (e, t) => Yr(t) ? Xr(e, t.value) : bt(t) ? {
  [`Map(${t.size})`]: [...t.entries()].reduce(
    (n, [s, r], o) => (n[Xn(s, o) + " =>"] = r, n),
    {}
  )
} : An(t) ? {
  [`Set(${t.size})`]: [...t.values()].map((n) => Xn(n))
} : De(t) ? Xn(t) : Q(t) && !F(t) && !Kr(t) ? String(t) : t, Xn = (e, t = "") => {
  var n;
  return (
    // Symbol.description in es2019+ so we need to cast here to pass
    // the lib: es2016 check
    De(e) ? `Symbol(${(n = e.description) != null ? n : t})` : e
  );
};
let ye;
class Mi {
  // TODO isolatedDeclarations "__v_skip"
  constructor(t = !1) {
    this.detached = t, this._active = !0, this._on = 0, this.effects = [], this.cleanups = [], this._isPaused = !1, this._warnOnRun = !0, this.__v_skip = !0, !t && ye && (ye.active ? (this.parent = ye, this.index = (ye.scopes || (ye.scopes = [])).push(
      this
    ) - 1) : (this._active = !1, this._warnOnRun = !1));
  }
  get active() {
    return this._active;
  }
  pause() {
    if (this._active) {
      this._isPaused = !0;
      let t, n;
      if (this.scopes) {
        const s = this.scopes.slice();
        for (t = 0, n = s.length; t < n; t++)
          s[t].pause();
      }
      for (t = 0, n = this.effects.length; t < n; t++)
        this.effects[t].pause();
    }
  }
  /**
   * Resumes the effect scope, including all child scopes and effects.
   */
  resume() {
    if (this._active && this._isPaused) {
      this._isPaused = !1;
      let t, n;
      if (this.scopes) {
        const r = this.scopes.slice();
        for (t = 0, n = r.length; t < n; t++)
          r[t].resume();
      }
      const s = this.effects.slice();
      for (t = 0, n = s.length; t < n; t++)
        s[t].resume();
    }
  }
  run(t) {
    if (this._active) {
      const n = ye;
      try {
        return ye = this, t();
      } finally {
        ye = n;
      }
    }
  }
  /**
   * This should only be called on non-detached scopes
   * @internal
   */
  on() {
    ++this._on === 1 && (this.prevScope = ye, ye = this);
  }
  /**
   * This should only be called on non-detached scopes
   * @internal
   */
  off() {
    if (this._on > 0 && --this._on === 0) {
      if (ye === this)
        ye = this.prevScope;
      else {
        let t = ye;
        for (; t; ) {
          if (t.prevScope === this) {
            t.prevScope = this.prevScope;
            break;
          }
          t = t.prevScope;
        }
      }
      this.prevScope = void 0;
    }
  }
  stop(t) {
    if (this._active) {
      this._active = !1;
      let n, s;
      for (n = 0, s = this.effects.length; n < s; n++)
        this.effects[n].stop();
      for (this.effects.length = 0, n = 0, s = this.cleanups.length; n < s; n++)
        this.cleanups[n]();
      if (this.cleanups.length = 0, this.scopes) {
        const r = this.scopes.slice();
        for (n = 0, s = r.length; n < s; n++)
          r[n].stop(!0);
        this.scopes.length = 0;
      }
      if (!this.detached && this.parent && !t) {
        const r = this.parent.scopes.pop();
        r && r !== this && (this.parent.scopes[this.index] = r, r.index = this.index);
      }
      this.parent = void 0;
    }
  }
}
function Ri() {
  return ye;
}
let re;
const Qn = /* @__PURE__ */ new WeakSet();
class Qr {
  constructor(t) {
    this.fn = t, this.deps = void 0, this.depsTail = void 0, this.flags = 5, this.next = void 0, this.cleanup = void 0, this.scheduler = void 0, ye && (ye.active ? ye.effects.push(this) : this.flags &= -2);
  }
  pause() {
    this.flags |= 64;
  }
  resume() {
    this.flags & 64 && (this.flags &= -65, Qn.has(this) && (Qn.delete(this), this.trigger()));
  }
  /**
   * @internal
   */
  notify() {
    this.flags & 2 && !(this.flags & 32) || this.flags & 8 || eo(this);
  }
  run() {
    if (!(this.flags & 1))
      return this.fn();
    this.flags |= 2, tr(this), to(this);
    const t = re, n = ze;
    re = this, ze = !0;
    try {
      return this.fn();
    } finally {
      no(this), re = t, ze = n, this.flags &= -3;
    }
  }
  stop() {
    if (this.flags & 1) {
      for (let t = this.deps; t; t = t.nextDep)
        Ls(t);
      this.deps = this.depsTail = void 0, tr(this), this.onStop && this.onStop(), this.flags &= -2;
    }
  }
  trigger() {
    this.flags & 64 ? Qn.add(this) : this.scheduler ? this.scheduler() : this.runIfDirty();
  }
  /**
   * @internal
   */
  runIfDirty() {
    hs(this) && this.run();
  }
  get dirty() {
    return hs(this);
  }
}
let Zr = 0, Zt, en;
function eo(e, t = !1) {
  if (e.flags |= 8, t) {
    e.next = en, en = e;
    return;
  }
  e.next = Zt, Zt = e;
}
function Ms() {
  Zr++;
}
function Rs() {
  if (--Zr > 0)
    return;
  if (en) {
    let t = en;
    for (en = void 0; t; ) {
      const n = t.next;
      t.next = void 0, t.flags &= -9, t = n;
    }
  }
  let e;
  for (; Zt; ) {
    let t = Zt;
    for (Zt = void 0; t; ) {
      const n = t.next;
      if (t.next = void 0, t.flags &= -9, t.flags & 1)
        try {
          t.trigger();
        } catch (s) {
          e || (e = s);
        }
      t = n;
    }
  }
  if (e) throw e;
}
function to(e) {
  for (let t = e.deps; t; t = t.nextDep)
    t.version = -1, t.prevActiveLink = t.dep.activeLink, t.dep.activeLink = t;
}
function no(e) {
  let t, n = e.depsTail, s = n;
  for (; s; ) {
    const r = s.prevDep;
    s.version === -1 ? (s === n && (n = r), Ls(s), Li(s)) : t = s, s.dep.activeLink = s.prevActiveLink, s.prevActiveLink = void 0, s = r;
  }
  e.deps = t, e.depsTail = n;
}
function hs(e) {
  for (let t = e.deps; t; t = t.nextDep)
    if (t.dep.version !== t.version || t.dep.computed && (so(t.dep.computed) || t.dep.version !== t.version))
      return !0;
  return !!e._dirty;
}
function so(e) {
  if (e.flags & 4 && !(e.flags & 16) || (e.flags &= -17, e.globalVersion === ln) || (e.globalVersion = ln, !e.isSSR && e.flags & 128 && (!e.deps && !e._dirty || !hs(e))))
    return;
  e.flags |= 2;
  const t = e.dep, n = re, s = ze;
  re = e, ze = !0;
  try {
    to(e);
    const r = e.fn(e._value);
    (t.version === 0 || Ye(r, e._value)) && (e.flags |= 128, e._value = r, t.version++);
  } catch (r) {
    throw t.version++, r;
  } finally {
    re = n, ze = s, no(e), e.flags &= -3;
  }
}
function Ls(e, t = !1) {
  const { dep: n, prevSub: s, nextSub: r } = e;
  if (s && (s.nextSub = r, e.prevSub = void 0), r && (r.prevSub = s, e.nextSub = void 0), n.subs === e && (n.subs = s, !s && n.computed)) {
    n.computed.flags &= -5;
    for (let o = n.computed.deps; o; o = o.nextDep)
      Ls(o, !0);
  }
  !t && !--n.sc && n.map && n.map.delete(n.key);
}
function Li(e) {
  const { prevDep: t, nextDep: n } = e;
  t && (t.nextDep = n, e.prevDep = void 0), n && (n.prevDep = t, e.nextDep = void 0);
}
let ze = !0;
const ro = [];
function Qe() {
  ro.push(ze), ze = !1;
}
function Ze() {
  const e = ro.pop();
  ze = e === void 0 ? !0 : e;
}
function tr(e) {
  const { cleanup: t } = e;
  if (e.cleanup = void 0, t) {
    const n = re;
    re = void 0;
    try {
      t();
    } finally {
      re = n;
    }
  }
}
let ln = 0;
class ji {
  constructor(t, n) {
    this.sub = t, this.dep = n, this.version = n.version, this.nextDep = this.prevDep = this.nextSub = this.prevSub = this.prevActiveLink = void 0;
  }
}
class js {
  // TODO isolatedDeclarations "__v_skip"
  constructor(t) {
    this.computed = t, this.version = 0, this.activeLink = void 0, this.subs = void 0, this.map = void 0, this.key = void 0, this.sc = 0, this.__v_skip = !0;
  }
  track(t) {
    if (!re || !ze || re === this.computed)
      return;
    let n = this.activeLink;
    if (n === void 0 || n.sub !== re)
      n = this.activeLink = new ji(re, this), re.deps ? (n.prevDep = re.depsTail, re.depsTail.nextDep = n, re.depsTail = n) : re.deps = re.depsTail = n, oo(n);
    else if (n.version === -1 && (n.version = this.version, n.nextDep)) {
      const s = n.nextDep;
      s.prevDep = n.prevDep, n.prevDep && (n.prevDep.nextDep = s), n.prevDep = re.depsTail, n.nextDep = void 0, re.depsTail.nextDep = n, re.depsTail = n, re.deps === n && (re.deps = s);
    }
    return n;
  }
  trigger(t) {
    this.version++, ln++, this.notify(t);
  }
  notify(t) {
    Ms();
    try {
      for (let n = this.subs; n; n = n.prevSub)
        n.sub.notify() && n.sub.dep.notify();
    } finally {
      Rs();
    }
  }
}
function oo(e) {
  if (e.dep.sc++, e.sub.flags & 4) {
    const t = e.dep.computed;
    if (t && !e.dep.subs) {
      t.flags |= 20;
      for (let s = t.deps; s; s = s.nextDep)
        oo(s);
    }
    const n = e.dep.subs;
    n !== e && (e.prevSub = n, n && (n.nextSub = e)), e.dep.subs = e;
  }
}
const ps = /* @__PURE__ */ new WeakMap(), Pt = /* @__PURE__ */ Symbol(
  ""
), gs = /* @__PURE__ */ Symbol(
  ""
), cn = /* @__PURE__ */ Symbol(
  ""
);
function xe(e, t, n) {
  if (ze && re) {
    let s = ps.get(e);
    s || ps.set(e, s = /* @__PURE__ */ new Map());
    let r = s.get(n);
    r || (s.set(n, r = new js()), r.map = s, r.key = n), r.track();
  }
}
function ut(e, t, n, s, r, o) {
  const i = ps.get(e);
  if (!i) {
    ln++;
    return;
  }
  const l = (c) => {
    c && c.trigger();
  };
  if (Ms(), t === "clear")
    i.forEach(l);
  else {
    const c = F(e), u = c && Os(n);
    if (c && n === "length") {
      const f = Number(s);
      i.forEach((h, b) => {
        (b === "length" || b === cn || !De(b) && b >= f) && l(h);
      });
    } else
      switch ((n !== void 0 || i.has(void 0)) && l(i.get(n)), u && l(i.get(cn)), t) {
        case "add":
          c ? u && l(i.get("length")) : (l(i.get(Pt)), bt(e) && l(i.get(gs)));
          break;
        case "delete":
          c || (l(i.get(Pt)), bt(e) && l(i.get(gs)));
          break;
        case "set":
          bt(e) && l(i.get(Pt));
          break;
      }
  }
  Rs();
}
function Rt(e) {
  const t = /* @__PURE__ */ X(e);
  return t === e || (xe(t, "iterate", cn), /* @__PURE__ */ Me(e)) ? t : /* @__PURE__ */ et(e) ? /* @__PURE__ */ xt(e) ? t.map((n) => vt(Re(n))) : t.map(vt) : t.map(Re);
}
function Wn(e) {
  return xe(e = /* @__PURE__ */ X(e), "iterate", cn), e;
}
function Ge(e, t) {
  return /* @__PURE__ */ et(e) ? vt(/* @__PURE__ */ xt(e) ? Re(t) : t) : Re(t);
}
const $i = {
  __proto__: null,
  [Symbol.iterator]() {
    return Zn(this, Symbol.iterator, (e) => Ge(this, e));
  },
  concat(...e) {
    return Rt(this).concat(
      ...e.map((t) => F(t) ? Rt(t) : t)
    );
  },
  entries() {
    return Zn(this, "entries", (e) => (e[1] = Ge(this, e[1]), e));
  },
  every(e, t) {
    return ot(this, "every", e, t, void 0, arguments);
  },
  filter(e, t) {
    return ot(
      this,
      "filter",
      e,
      t,
      (n) => n.map((s) => Ge(this, s)),
      arguments
    );
  },
  find(e, t) {
    return ot(
      this,
      "find",
      e,
      t,
      (n) => Ge(this, n),
      arguments
    );
  },
  findIndex(e, t) {
    return ot(this, "findIndex", e, t, void 0, arguments);
  },
  findLast(e, t) {
    return ot(
      this,
      "findLast",
      e,
      t,
      (n) => Ge(this, n),
      arguments
    );
  },
  findLastIndex(e, t) {
    return ot(this, "findLastIndex", e, t, void 0, arguments);
  },
  // flat, flatMap could benefit from ARRAY_ITERATE but are not straight-forward to implement
  forEach(e, t) {
    return ot(this, "forEach", e, t, void 0, arguments);
  },
  includes(...e) {
    return es(this, "includes", e);
  },
  indexOf(...e) {
    return es(this, "indexOf", e);
  },
  join(e) {
    return Rt(this).join(e);
  },
  // keys() iterator only reads `length`, no optimization required
  lastIndexOf(...e) {
    return es(this, "lastIndexOf", e);
  },
  map(e, t) {
    return ot(this, "map", e, t, void 0, arguments);
  },
  pop() {
    return Kt(this, "pop");
  },
  push(...e) {
    return Kt(this, "push", e);
  },
  reduce(e, ...t) {
    return sr(this, "reduce", e, t);
  },
  reduceRight(e, ...t) {
    return sr(this, "reduceRight", e, t);
  },
  shift() {
    return Kt(this, "shift");
  },
  // slice could use ARRAY_ITERATE but also seems to beg for range tracking
  some(e, t) {
    return ot(this, "some", e, t, void 0, arguments);
  },
  splice(...e) {
    return Kt(this, "splice", e);
  },
  toReversed() {
    return Rt(this).toReversed();
  },
  toSorted(e) {
    return Rt(this).toSorted(e);
  },
  toSpliced(...e) {
    return Rt(this).toSpliced(...e);
  },
  unshift(...e) {
    return Kt(this, "unshift", e);
  },
  values() {
    return Zn(this, "values", (e) => Ge(this, e));
  }
};
function Zn(e, t, n) {
  const s = Wn(e), r = s[t], o = r.call(s);
  if (s !== e && !/* @__PURE__ */ Me(e)) {
    if (o[Symbol.toStringTag] === "Array Iterator") {
      const i = Object.create(Object.getPrototypeOf(o));
      return i.next = () => nr(o.next(), n), r !== io[t] && Object.defineProperties(i, {
        return: {
          get() {
            const l = o.return;
            return typeof l == "function" ? l.bind(o) : l;
          }
        },
        throw: {
          get() {
            const l = o.throw;
            return typeof l == "function" ? l.bind(o) : l;
          }
        }
      }), i;
    }
    o._next = o.next, o.next = () => nr(o._next(), n);
  }
  return o;
}
function nr(e, t) {
  return e.done || (e.value = t(e.value)), e;
}
const io = Array.prototype;
function ot(e, t, n, s, r, o) {
  const i = Wn(e), l = i !== e && !/* @__PURE__ */ Me(e), c = i[t];
  if (c !== io[t]) {
    const h = c.apply(e, o);
    return l ? Re(h) : h;
  }
  let u = n;
  i !== e && (l ? u = function(h, b) {
    return n.call(this, Ge(e, h), b, e);
  } : n.length > 2 && (u = function(h, b) {
    return n.call(this, h, b, e);
  }));
  const f = c.call(i, u, s);
  return l && r ? r(f) : f;
}
function sr(e, t, n, s) {
  const r = Wn(e), o = r !== e && !/* @__PURE__ */ Me(e);
  let i = n, l = !1;
  r !== e && (o ? (l = s.length === 0, i = function(u, f, h) {
    return l && (l = !1, u = Ge(e, u)), n.call(this, u, Ge(e, f), h, e);
  }) : n.length > 3 && (i = function(u, f, h) {
    return n.call(this, u, f, h, e);
  }));
  const c = r[t](i, ...s);
  return l ? Ge(e, c) : c;
}
function es(e, t, n) {
  const s = /* @__PURE__ */ X(e);
  xe(s, "iterate", cn);
  const r = s[t](...n);
  return (r === -1 || r === !1) && /* @__PURE__ */ zs(n[0]) ? (n[0] = /* @__PURE__ */ X(n[0]), s[t](...n)) : r;
}
function Kt(e, t, n = []) {
  Qe(), Ms();
  const s = (/* @__PURE__ */ X(e))[t].apply(e, n);
  return Rs(), Ze(), s;
}
const Ni = /* @__PURE__ */ Es("__proto__,__v_isRef,__isVue"), lo = /* @__PURE__ */ new Set(
  /* @__PURE__ */ Object.getOwnPropertyNames(Symbol).filter((e) => e !== "arguments" && e !== "caller").map((e) => Symbol[e]).filter(De)
);
function Fi(e) {
  De(e) || (e = String(e));
  const t = /* @__PURE__ */ X(this);
  return xe(t, "has", e), t.hasOwnProperty(e);
}
class co {
  constructor(t = !1, n = !1) {
    this._isReadonly = t, this._isShallow = n;
  }
  get(t, n, s) {
    if (n === "__v_skip") return t.__v_skip;
    const r = this._isReadonly, o = this._isShallow;
    if (n === "__v_isReactive")
      return !r;
    if (n === "__v_isReadonly")
      return r;
    if (n === "__v_isShallow")
      return o;
    if (n === "__v_raw")
      return s === (r ? o ? qi : ho : o ? fo : uo).get(t) || // receiver is not the reactive proxy, but has the same prototype
      // this means the receiver is a user proxy of the reactive proxy
      Object.getPrototypeOf(t) === Object.getPrototypeOf(s) ? t : void 0;
    const i = F(t);
    if (!r) {
      let c;
      if (i && (c = $i[n]))
        return c;
      if (n === "hasOwnProperty")
        return Fi;
    }
    const l = Reflect.get(
      t,
      n,
      // if this is a proxy wrapping a ref, return methods using the raw ref
      // as receiver so that we don't have to call `toRaw` on the ref in all
      // its class methods
      /* @__PURE__ */ _e(t) ? t : s
    );
    if ((De(n) ? lo.has(n) : Ni(n)) || (r || xe(t, "get", n), o))
      return l;
    if (/* @__PURE__ */ _e(l)) {
      const c = i && Os(n) ? l : l.value;
      return r && Q(c) ? /* @__PURE__ */ bs(c) : c;
    }
    return Q(l) ? r ? /* @__PURE__ */ bs(l) : /* @__PURE__ */ Ns(l) : l;
  }
}
class ao extends co {
  constructor(t = !1) {
    super(!1, t);
  }
  set(t, n, s, r) {
    let o = t[n];
    const i = F(t) && Os(n);
    if (!this._isShallow) {
      const u = /* @__PURE__ */ et(o);
      if (!/* @__PURE__ */ Me(s) && !/* @__PURE__ */ et(s) && (o = /* @__PURE__ */ X(o), s = /* @__PURE__ */ X(s)), !i && /* @__PURE__ */ _e(o) && !/* @__PURE__ */ _e(s))
        return u || (o.value = s), !0;
    }
    const l = i ? Number(n) < t.length : J(t, n), c = Reflect.set(
      t,
      n,
      s,
      /* @__PURE__ */ _e(t) ? t : r
    );
    return t === /* @__PURE__ */ X(r) && c && (l ? Ye(s, o) && ut(t, "set", n, s) : ut(t, "add", n, s)), c;
  }
  deleteProperty(t, n) {
    const s = J(t, n);
    t[n];
    const r = Reflect.deleteProperty(t, n);
    return r && s && ut(t, "delete", n, void 0), r;
  }
  has(t, n) {
    const s = Reflect.has(t, n);
    return (!De(n) || !lo.has(n)) && xe(t, "has", n), s;
  }
  ownKeys(t) {
    return xe(
      t,
      "iterate",
      F(t) ? "length" : Pt
    ), Reflect.ownKeys(t);
  }
}
class zi extends co {
  constructor(t = !1) {
    super(!0, t);
  }
  set(t, n) {
    return !0;
  }
  deleteProperty(t, n) {
    return !0;
  }
}
const Di = /* @__PURE__ */ new ao(), Hi = /* @__PURE__ */ new zi(), Wi = /* @__PURE__ */ new ao(!0);
const ms = (e) => e, yn = (e) => Reflect.getPrototypeOf(e);
function Bi(e, t, n) {
  return function(...s) {
    const r = this.__v_raw, o = /* @__PURE__ */ X(r), i = bt(o), l = e === "entries" || e === Symbol.iterator && i, c = e === "keys" && i, u = r[e](...s), f = n ? ms : t ? vt : Re;
    return !t && xe(
      o,
      "iterate",
      c ? gs : Pt
    ), he(
      // inheriting all iterator properties
      Object.create(u),
      {
        // iterator protocol
        next() {
          const { value: h, done: b } = u.next();
          return b ? { value: h, done: b } : {
            value: l ? [f(h[0]), f(h[1])] : f(h),
            done: b
          };
        }
      }
    );
  };
}
function xn(e) {
  return function(...t) {
    return e === "delete" ? !1 : e === "clear" ? void 0 : this;
  };
}
function Vi(e, t) {
  const n = {
    get(r) {
      const o = this.__v_raw, i = /* @__PURE__ */ X(o), l = /* @__PURE__ */ X(r);
      e || (Ye(r, l) && xe(i, "get", r), xe(i, "get", l));
      const { has: c } = yn(i), u = t ? ms : e ? vt : Re;
      if (c.call(i, r))
        return u(o.get(r));
      if (c.call(i, l))
        return u(o.get(l));
      o !== i && o.get(r);
    },
    get size() {
      const r = this.__v_raw;
      return !e && xe(/* @__PURE__ */ X(r), "iterate", Pt), r.size;
    },
    has(r) {
      const o = this.__v_raw, i = /* @__PURE__ */ X(o), l = /* @__PURE__ */ X(r);
      return e || (Ye(r, l) && xe(i, "has", r), xe(i, "has", l)), r === l ? o.has(r) : o.has(r) || o.has(l);
    },
    forEach(r, o) {
      const i = this, l = i.__v_raw, c = /* @__PURE__ */ X(l), u = t ? ms : e ? vt : Re;
      return !e && xe(c, "iterate", Pt), l.forEach((f, h) => r.call(o, u(f), u(h), i));
    }
  };
  return he(
    n,
    e ? {
      add: xn("add"),
      set: xn("set"),
      delete: xn("delete"),
      clear: xn("clear")
    } : {
      add(r) {
        const o = /* @__PURE__ */ X(this), i = yn(o), l = /* @__PURE__ */ X(r), c = !t && !/* @__PURE__ */ Me(r) && !/* @__PURE__ */ et(r) ? l : r;
        return i.has.call(o, c) || Ye(r, c) && i.has.call(o, r) || Ye(l, c) && i.has.call(o, l) || (o.add(c), ut(o, "add", c, c)), this;
      },
      set(r, o) {
        !t && !/* @__PURE__ */ Me(o) && !/* @__PURE__ */ et(o) && (o = /* @__PURE__ */ X(o));
        const i = /* @__PURE__ */ X(this), { has: l, get: c } = yn(i);
        let u = l.call(i, r);
        u || (r = /* @__PURE__ */ X(r), u = l.call(i, r));
        const f = c.call(i, r);
        return i.set(r, o), u ? Ye(o, f) && ut(i, "set", r, o) : ut(i, "add", r, o), this;
      },
      delete(r) {
        const o = /* @__PURE__ */ X(this), { has: i, get: l } = yn(o);
        let c = i.call(o, r);
        c || (r = /* @__PURE__ */ X(r), c = i.call(o, r)), l && l.call(o, r);
        const u = o.delete(r);
        return c && ut(o, "delete", r, void 0), u;
      },
      clear() {
        const r = /* @__PURE__ */ X(this), o = r.size !== 0, i = r.clear();
        return o && ut(
          r,
          "clear",
          void 0,
          void 0
        ), i;
      }
    }
  ), [
    "keys",
    "values",
    "entries",
    Symbol.iterator
  ].forEach((r) => {
    n[r] = Bi(r, e, t);
  }), n;
}
function $s(e, t) {
  const n = Vi(e, t);
  return (s, r, o) => r === "__v_isReactive" ? !e : r === "__v_isReadonly" ? e : r === "__v_isShallow" ? t : r === "__v_raw" ? s : Reflect.get(
    J(n, r) && r in s ? n : s,
    r,
    o
  );
}
const Ui = {
  get: /* @__PURE__ */ $s(!1, !1)
}, Ki = {
  get: /* @__PURE__ */ $s(!1, !0)
}, Gi = {
  get: /* @__PURE__ */ $s(!0, !1)
};
const uo = /* @__PURE__ */ new WeakMap(), fo = /* @__PURE__ */ new WeakMap(), ho = /* @__PURE__ */ new WeakMap(), qi = /* @__PURE__ */ new WeakMap();
function Ji(e) {
  switch (e) {
    case "Object":
    case "Array":
      return 1;
    case "Map":
    case "Set":
    case "WeakMap":
    case "WeakSet":
      return 2;
    default:
      return 0;
  }
}
// @__NO_SIDE_EFFECTS__
function Ns(e) {
  return /* @__PURE__ */ et(e) ? e : Fs(
    e,
    !1,
    Di,
    Ui,
    uo
  );
}
// @__NO_SIDE_EFFECTS__
function Yi(e) {
  return Fs(
    e,
    !1,
    Wi,
    Ki,
    fo
  );
}
// @__NO_SIDE_EFFECTS__
function bs(e) {
  return Fs(
    e,
    !0,
    Hi,
    Gi,
    ho
  );
}
function Fs(e, t, n, s, r) {
  if (!Q(e) || e.__v_raw && !(t && e.__v_isReactive) || e.__v_skip || !Object.isExtensible(e))
    return e;
  const o = r.get(e);
  if (o)
    return o;
  const i = Ji(vi(e));
  if (i === 0)
    return e;
  const l = new Proxy(
    e,
    i === 2 ? s : n
  );
  return r.set(e, l), l;
}
// @__NO_SIDE_EFFECTS__
function xt(e) {
  return /* @__PURE__ */ et(e) ? /* @__PURE__ */ xt(e.__v_raw) : !!(e && e.__v_isReactive);
}
// @__NO_SIDE_EFFECTS__
function et(e) {
  return !!(e && e.__v_isReadonly);
}
// @__NO_SIDE_EFFECTS__
function Me(e) {
  return !!(e && e.__v_isShallow);
}
// @__NO_SIDE_EFFECTS__
function zs(e) {
  return e ? !!e.__v_raw : !1;
}
// @__NO_SIDE_EFFECTS__
function X(e) {
  const t = e && e.__v_raw;
  return t ? /* @__PURE__ */ X(t) : e;
}
function Xi(e) {
  return !J(e, "__v_skip") && Object.isExtensible(e) && qr(e, "__v_skip", !0), e;
}
const Re = (e) => Q(e) ? /* @__PURE__ */ Ns(e) : e, vt = (e) => Q(e) ? /* @__PURE__ */ bs(e) : e;
// @__NO_SIDE_EFFECTS__
function _e(e) {
  return e ? e.__v_isRef === !0 : !1;
}
// @__NO_SIDE_EFFECTS__
function Lt(e) {
  return Qi(e, !1);
}
function Qi(e, t) {
  return /* @__PURE__ */ _e(e) ? e : new Zi(e, t);
}
class Zi {
  constructor(t, n) {
    this.dep = new js(), this.__v_isRef = !0, this.__v_isShallow = !1, this._rawValue = n ? t : /* @__PURE__ */ X(t), this._value = n ? t : Re(t), this.__v_isShallow = n;
  }
  get value() {
    return this.dep.track(), this._value;
  }
  set value(t) {
    const n = this._rawValue, s = this.__v_isShallow || /* @__PURE__ */ Me(t) || /* @__PURE__ */ et(t);
    t = s ? t : /* @__PURE__ */ X(t), Ye(t, n) && (this._rawValue = t, this._value = s ? t : Re(t), this.dep.trigger());
  }
}
function Ie(e) {
  return /* @__PURE__ */ _e(e) ? e.value : e;
}
const el = {
  get: (e, t, n) => t === "__v_raw" ? e : Ie(Reflect.get(e, t, n)),
  set: (e, t, n, s) => {
    const r = e[t];
    return /* @__PURE__ */ _e(r) && !/* @__PURE__ */ _e(n) ? (r.value = n, !0) : Reflect.set(e, t, n, s);
  }
};
function po(e) {
  return /* @__PURE__ */ xt(e) ? e : new Proxy(e, el);
}
class tl {
  constructor(t, n, s) {
    this.fn = t, this.setter = n, this._value = void 0, this.dep = new js(this), this.__v_isRef = !0, this.deps = void 0, this.depsTail = void 0, this.flags = 16, this.globalVersion = ln - 1, this.next = void 0, this.effect = this, this.__v_isReadonly = !n, this.isSSR = s;
  }
  /**
   * @internal
   */
  notify() {
    if (this.flags |= 16, !(this.flags & 8) && // avoid infinite self recursion
    re !== this)
      return eo(this, !0), !0;
  }
  get value() {
    const t = this.dep.track();
    return so(this), t && (t.version = this.dep.version), this._value;
  }
  set value(t) {
    this.setter && this.setter(t);
  }
}
// @__NO_SIDE_EFFECTS__
function nl(e, t, n = !1) {
  let s, r;
  return H(e) ? s = e : (s = e.get, r = e.set), new tl(s, r, n);
}
const vn = {}, Tn = /* @__PURE__ */ new WeakMap();
let Tt;
function sl(e, t = !1, n = Tt) {
  if (n) {
    let s = Tn.get(n);
    s || Tn.set(n, s = []), s.push(e);
  }
}
function rl(e, t, n = ne) {
  const { immediate: s, deep: r, once: o, scheduler: i, augmentJob: l, call: c } = n, u = (O) => r ? O : /* @__PURE__ */ Me(O) || r === !1 || r === 0 ? ft(O, 1) : ft(O);
  let f, h, b, w, L = !1, C = !1;
  if (/* @__PURE__ */ _e(e) ? (h = () => e.value, L = /* @__PURE__ */ Me(e)) : /* @__PURE__ */ xt(e) ? (h = () => u(e), L = !0) : F(e) ? (C = !0, L = e.some((O) => /* @__PURE__ */ xt(O) || /* @__PURE__ */ Me(O)), h = () => e.map((O) => {
    if (/* @__PURE__ */ _e(O))
      return O.value;
    if (/* @__PURE__ */ xt(O))
      return u(O);
    if (H(O))
      return c ? c(O, 2) : O();
  })) : H(e) ? t ? h = c ? () => c(e, 2) : e : h = () => {
    if (b) {
      Qe();
      try {
        b();
      } finally {
        Ze();
      }
    }
    const O = Tt;
    Tt = f;
    try {
      return c ? c(e, 3, [w]) : e(w);
    } finally {
      Tt = O;
    }
  } : h = Xe, t && r) {
    const O = h, B = r === !0 ? 1 / 0 : r;
    h = () => ft(O(), B);
  }
  const $ = Ri(), D = () => {
    f.stop(), $ && $.active && Ps($.effects, f);
  };
  if (o && t) {
    const O = t;
    t = (...B) => {
      try {
        return O(...B);
      } finally {
        D();
      }
    };
  }
  let j = C ? new Array(e.length).fill(vn) : vn;
  const W = (O) => {
    if (!(!(f.flags & 1) || !f.dirty && !O))
      if (t) {
        const B = f.run();
        if (O || r || L || (C ? B.some((N, G) => Ye(N, j[G])) : Ye(B, j))) {
          const N = Tt;
          Qe();
          try {
            b && b(), Tt = f;
            const G = [
              B,
              // pass undefined as the old value when it's changed for the first time
              j === vn ? void 0 : C && j[0] === vn ? [] : j,
              w
            ];
            j = B, c ? c(t, 3, G) : (
              // @ts-expect-error
              t(...G)
            );
          } finally {
            Tt = N, Ze();
          }
        }
      } else
        f.run();
  };
  return l && l(W), f = new Qr(h), f.scheduler = i ? () => i(W, !1) : W, w = (O) => sl(O, !1, f), b = f.onStop = () => {
    const O = Tn.get(f);
    if (O) {
      if (c)
        c(O, 4);
      else
        for (const B of O) B();
      Tn.delete(f);
    }
  }, t ? s ? W(!0) : j = f.run() : i ? i(W.bind(null, !0), !0) : f.run(), D.pause = f.pause.bind(f), D.resume = f.resume.bind(f), D.stop = D, D;
}
function ft(e, t = 1 / 0, n) {
  if (t <= 0 || !Q(e) || e.__v_skip || (n = n || /* @__PURE__ */ new Map(), (n.get(e) || 0) >= t))
    return e;
  if (n.set(e, t), t--, /* @__PURE__ */ _e(e))
    ft(e.value, t, n);
  else if (F(e))
    for (let s = 0; s < e.length; s++)
      ft(e[s], t, n);
  else if (An(e) || bt(e))
    e.forEach((s) => {
      ft(s, t, n);
    });
  else if (Kr(e)) {
    for (const s in e)
      ft(e[s], t, n);
    for (const s of Object.getOwnPropertySymbols(e))
      Object.prototype.propertyIsEnumerable.call(e, s) && ft(e[s], t, n);
  }
  return e;
}
function mn(e, t, n, s) {
  try {
    return s ? e(...s) : e();
  } catch (r) {
    Bn(r, t, n);
  }
}
function He(e, t, n, s) {
  if (H(e)) {
    const r = mn(e, t, n, s);
    return r && Vr(r) && r.catch((o) => {
      Bn(o, t, n, void 0, !0);
    }), r;
  }
  if (F(e)) {
    const r = [];
    for (let o = 0; o < e.length; o++)
      r.push(He(e[o], t, n, s));
    return r;
  }
}
function Bn(e, t, n, s = !0, r = !1) {
  const o = t ? t.vnode : null, { errorHandler: i, throwUnhandledErrorInProduction: l } = t && t.appContext.config || ne;
  if (t) {
    let c = t.parent;
    const u = t.proxy, f = `https://vuejs.org/error-reference/#runtime-${n}`;
    for (; c; ) {
      const h = c.ec;
      if (h && !c.isUnmounted && // Async failures may outlive stopped scopes whose unmount effects
      // are deferred or discarded by Suspense.
      (!r || c.scope.active)) {
        for (let b = 0; b < h.length; b++)
          if (h[b](e, u, f) === !1)
            return;
      }
      c = c.parent;
    }
    if (i) {
      Qe(), mn(i, null, 10, [
        e,
        u,
        f
      ]), Ze();
      return;
    }
  }
  ol(e, n, o, s, l);
}
function ol(e, t, n, s = !0, r = !1) {
  if (r)
    throw e;
  console.error(e);
}
const Se = [];
let Ne = -1;
const qe = [];
let mt = null, $t = 0;
const go = /* @__PURE__ */ Promise.resolve();
let En = null;
function il(e) {
  const t = En || go;
  return e ? t.then(this ? e.bind(this) : e) : t;
}
function ll(e) {
  let t = Ne + 1, n = Se.length;
  for (; t < n; ) {
    const s = t + n >>> 1, r = Se[s], o = an(r);
    o < e || o === e && r.flags & 2 ? t = s + 1 : n = s;
  }
  return t;
}
function Ds(e) {
  if (!(e.flags & 1)) {
    const t = an(e), n = Se[Se.length - 1];
    !n || // fast path when the job id is larger than the tail
    !(e.flags & 2) && t >= an(n) ? Se.push(e) : Se.splice(ll(t), 0, e), e.flags |= 1, mo();
  }
}
function mo() {
  En || (En = go.then(xo));
}
function cl(e) {
  if (!F(e))
    mt && e.id === -1 ? mt.splice($t + 1, 0, e) : e.flags & 1 || (qe.push(e), e.flags |= 1);
  else
    for (let t = 0; t < e.length; t++)
      qe.push(e[t]);
  mo();
}
function bo(e, t, n, s = Ne + 1) {
  for (; s < Se.length; s++) {
    const r = Se[s];
    if (r && r.flags & 2) {
      if (e && r.id !== e.uid || t !== void 0 && !(r.id >= t))
        continue;
      Se.splice(s, 1), s = e ? s - 1 : Ne, r.flags & 4 && (r.flags &= -2), r(), r.flags & 4 || (r.flags &= -2);
    }
  }
}
function yo(e, t, n = 0) {
  if (qe.length) {
    let s = qe, r = 0;
    if (t) {
      s = [];
      for (let i = 0; i < qe.length; i++) {
        const l = qe[i];
        (l.i !== void 0 ? !l.i || l.i.uid < n : t.has(l)) ? qe[r++] = l : s.push(l);
      }
    }
    const o = [...new Set(s)].sort((i, l) => an(i) - an(l));
    if (qe.length = r, mt) {
      for (let i = 0; i < o.length; i++)
        mt.push(o[i]);
      return;
    }
    for (mt = o, $t = 0; $t < mt.length; $t++) {
      const i = mt[$t];
      i.flags & 4 && (i.flags &= -2), i.flags & 8 || i(), i.flags &= -2;
    }
    mt = null, $t = 0;
  }
}
let ts = !1;
function al(e, t) {
  if (ts) {
    e();
    return;
  }
  const n = t === void 0 ? void 0 : new Set(qe);
  e(), ts = !0;
  try {
    bo(void 0, t), yo(void 0, n, t);
  } finally {
    ts = !1;
  }
}
const an = (e) => e.id == null ? e.flags & 2 ? -1 : 1 / 0 : e.id;
function xo(e) {
  try {
    for (Ne = 0; Ne < Se.length; Ne++) {
      const t = Se[Ne];
      t && !(t.flags & 8) && (t.flags & 4 && (t.flags &= -2), mn(
        t,
        t.i,
        t.i ? 15 : 14
      ), t.flags & 4 || (t.flags &= -2));
    }
  } finally {
    for (; Ne < Se.length; Ne++) {
      const t = Se[Ne];
      t && (t.flags &= -2);
    }
    Ne = -1, Se.length = 0, yo(), En = null, (Se.length || qe.length) && xo();
  }
}
let ve = null, vo = null;
function Pn(e) {
  const t = ve;
  return ve = e, vo = e && e.type.__scopeId || null, t;
}
function Nt(e, t = ve, n) {
  if (!t || e._n)
    return e;
  const s = (...r) => {
    s._d && Mn(-1);
    const o = Pn(t), i = dt.length;
    let l;
    try {
      l = e(...r);
    } finally {
      for (let c = dt.length; c > i; c--) Us();
      Pn(o), s._d && Mn(1);
    }
    return l;
  };
  return s._n = !0, s._c = !0, s._d = !0, s;
}
function ul(e, t) {
  if (ve === null)
    return e;
  const n = Gn(ve), s = e.dirs || (e.dirs = []);
  for (let r = 0; r < t.length; r++) {
    let [o, i, l, c = ne] = t[r];
    o && (H(o) && (o = {
      mounted: o,
      updated: o
    }), o.deep && ft(i), s.push({
      dir: o,
      instance: n,
      value: i,
      oldValue: void 0,
      arg: l,
      modifiers: c
    }));
  }
  return e;
}
function kt(e, t, n, s) {
  let r = e;
  if (s === "beforeMount")
    e._mount = e;
  else if (s === "beforeUpdate") {
    const l = t && t._mount;
    l && (l._latest = e, e._mount = l);
  } else if (s === "mounted") {
    const l = e._latest;
    e._mount = e._latest = void 0, l && (r = l, l._mount = void 0);
  }
  const o = r.dirs, i = t && t.dirs;
  for (let l = 0; l < o.length; l++) {
    let c = o[l];
    i ? c.oldValue = i[l].value : s === "mounted" && c.oldValue !== void 0 && (c = he({}, c, { oldValue: void 0 }));
    let u = c.dir[s];
    u && (Qe(), He(u, n, 8, [
      e.el,
      c,
      r,
      t
    ]), Ze());
  }
}
function fl(e, t) {
  if (Ce) {
    let n = Ce.provides;
    const s = Ce.parent && Ce.parent.provides;
    s === n && (n = Ce.provides = Object.create(s)), n[e] = t;
  }
}
function tn(e, t, n = !1) {
  const s = cc();
  if (s || Dt) {
    let r = Dt ? Dt._context.provides : s ? s.parent == null || s.ce ? s.vnode.appContext && s.vnode.appContext.provides : s.parent.provides : void 0;
    if (r && e in r)
      return r[e];
    if (arguments.length > 1)
      return n && H(t) ? t.call(s && s.proxy) : t;
  }
}
const dl = /* @__PURE__ */ Symbol.for("v-scx"), hl = () => tn(dl);
function ns(e, t, n) {
  return _o(e, t, n);
}
function _o(e, t, n = ne) {
  const { immediate: s, deep: r, flush: o, once: i } = n, l = he({}, n), c = t && s || !t && o !== "post";
  let u;
  if (pn) {
    if (o === "sync") {
      const w = hl();
      u = w.__watcherHandles || (w.__watcherHandles = []);
    } else if (!c) {
      const w = () => {
      };
      return w.stop = Xe, w.resume = Xe, w.pause = Xe, w;
    }
  }
  const f = Ce;
  l.call = (w, L, C) => He(w, f, L, C);
  let h = !1;
  o === "post" ? l.scheduler = (w) => {
    w.i = f, ke(w, f && f.suspense);
  } : o !== "sync" && (h = !0, l.scheduler = (w, L) => {
    L ? w() : Ds(w);
  }), l.augmentJob = (w) => {
    t && (w.flags |= 4), h && (w.flags |= 2, f && (w.id = f.uid, w.i = f));
  };
  const b = rl(e, t, l);
  return pn && (u ? u.push(b) : c && b()), b;
}
function pl(e, t, n) {
  const s = this.proxy, r = ce(e) ? e.includes(".") ? wo(s, e) : () => s[e] : e.bind(s, s);
  let o;
  H(t) ? o = t : (o = t.handler, n = t);
  const i = bn(this), l = _o(r, o.bind(s), n);
  return i(), l;
}
function wo(e, t) {
  const n = t.split(".");
  return () => {
    let s = e;
    for (let r = 0; r < n.length && s; r++)
      s = s[n[r]];
    return s;
  };
}
const gl = /* @__PURE__ */ Symbol("_vte"), Vn = (e) => e.__isTeleport, ss = /* @__PURE__ */ Symbol("_leaveCb");
function ml(e) {
  let t = e[0];
  if (e.length > 1) {
    for (const n of e)
      if (n.type !== Le) {
        t = n;
        break;
      }
  }
  return t;
}
function ko(e) {
  if (!Ws(e))
    return Vn(e.type) && e.children ? ml(e.children) : e;
  if (e.component)
    return e.component.subTree;
  const { shapeFlag: t, children: n } = e;
  if (n) {
    if (t & 16)
      return n[0];
    if (t & 32 && H(n.default))
      return n.default();
  }
}
function nn(e, t) {
  if (e.shapeFlag & 6 && e.component) {
    e.transition = t;
    const n = e.component.subTree;
    nn(
      Vn(n.type) && ko(n) || n,
      t
    );
  } else if (e.shapeFlag & 128) {
    const n = e.suspense;
    if (n) {
      const s = n.isHydrating ? n.pendingBranch : n.activeBranch;
      s && nn(s, t.clone(s));
    } else
      e.ssContent.transition = t.clone(e.ssContent), e.ssFallback.transition = t.clone(e.ssFallback);
  } else if (e.transition = t, e.type === fe && F(e.children)) {
    const n = Lo(e.children, !1);
    n && nn(n, t);
  }
}
function ys(e) {
  if (e.component)
    return ys(e.component.subTree);
  if (e.type === fe) {
    const t = F(e.children) && Lo(e.children, !1);
    return t ? ys(t) : void 0;
  }
  if (e.type !== Le)
    return e;
}
// @__NO_SIDE_EFFECTS__
function Hs(e, t) {
  return H(e) ? (
    // #8236: extend call and options.name access are considered side-effects
    // by Rollup, so we have to wrap it in a pure-annotated IIFE.
    he({ name: e.name }, t, { setup: e })
  ) : e;
}
function So(e) {
  e.ids = [e.ids[0] + e.ids[2]++ + "-", 0, 0];
}
function rr(e, t) {
  let n;
  return !!((n = Object.getOwnPropertyDescriptor(e, t)) && !n.configurable);
}
const On = /* @__PURE__ */ new WeakMap();
function sn(e, t, n, s, r = !1) {
  if (F(e)) {
    e.forEach(
      (C, $) => sn(
        C,
        t && (F(t) ? t[$] : t),
        n,
        s,
        r
      )
    );
    return;
  }
  if (zt(s) && !r) {
    s.shapeFlag & 512 && s.type.__asyncResolved && s.component.subTree.component && sn(e, t, n, s.component.subTree);
    return;
  }
  const o = s.shapeFlag & 4 ? Gn(s.component) : s.el, i = r ? null : o, { i: l, r: c } = e, u = t && t.r, f = l.refs === ne ? l.refs = {} : l.refs, h = l.setupState, b = /* @__PURE__ */ X(h), w = h === ne ? Br : (C) => rr(f, C) ? !1 : J(b, C), L = (C, $) => !($ && rr(f, $));
  if (u != null && u !== c) {
    if (or(t), ce(u))
      f[u] = null, w(u) && (h[u] = null);
    else if (/* @__PURE__ */ _e(u)) {
      const C = t;
      L(u, C.k) && (u.value = null), C.k && (f[C.k] = null);
    }
  }
  if (H(c))
    mn(c, l, 12, [i, f]);
  else {
    const C = ce(c), $ = /* @__PURE__ */ _e(c);
    if (C || $) {
      const D = () => {
        if (e.f) {
          const j = C ? w(c) ? h[c] : f[c] : L() || !e.k ? c.value : f[e.k];
          if (r)
            F(j) && Ps(j, o);
          else if (F(j))
            j.includes(o) || j.push(o);
          else if (C)
            f[c] = [o], w(c) && (h[c] = f[c]);
          else {
            const W = [o];
            L(c, e.k) && (c.value = W), e.k && (f[e.k] = W);
          }
        } else C ? (f[c] = i, w(c) && (h[c] = i)) : $ && (L(c, e.k) && (c.value = i), e.k && (f[e.k] = i));
      };
      if (i) {
        const j = () => {
          D(), On.delete(e);
        };
        j.id = -1, On.set(e, j), ke(j, n);
      } else
        or(e), D();
    }
  }
}
function or(e) {
  const t = On.get(e);
  t && (t.flags |= 8, On.delete(e));
}
Dn().requestIdleCallback;
Dn().cancelIdleCallback;
const zt = (e) => !!e.type.__asyncLoader, Ws = (e) => e.type.__isKeepAlive;
function bl(e, t) {
  Co(e, "a", t);
}
function yl(e, t) {
  Co(e, "da", t);
}
function Co(e, t, n = Ce) {
  const s = e.__wdc || (e.__wdc = () => {
    let r = n;
    for (; r; ) {
      if (r.isDeactivated)
        return;
      r = r.parent;
    }
    return e();
  });
  if (un(t, s, n), n) {
    let r = n.parent;
    for (; r && r.parent; )
      Ws(r.parent.vnode) && xl(s, t, n, r), r = r.parent;
  }
}
function xl(e, t, n, s) {
  const r = un(
    t,
    e,
    s,
    !0
    /* prepend */
  ), o = un(
    "um",
    () => Ps(s[t], r),
    n
  );
  o.flags = 16;
}
function un(e, t, n = Ce, s = !1) {
  if (n) {
    const r = n[e] || (n[e] = []), o = t.__weh || (t.__weh = (...i) => {
      Qe();
      const l = bn(n), c = He(t, n, e, i);
      return l(), Ze(), c;
    });
    return s ? r.unshift(o) : r.push(o), o;
  }
}
const ht = (e) => (t, n = Ce) => {
  (!pn || e === "sp") && un(e, (...s) => t(...s), n);
}, vl = ht("bm"), Ao = ht("m"), _l = ht(
  "bu"
), wl = ht("u"), To = ht(
  "bum"
), kl = ht("um"), Sl = ht(
  "sp"
), Cl = ht("rtg"), Al = ht("rtc");
function Tl(e, t = Ce) {
  un("ec", e, t);
}
const El = /* @__PURE__ */ Symbol.for("v-ndc");
function ir(e, t, n, s) {
  let r;
  const o = n, i = F(e);
  if (i || ce(e)) {
    const l = i && /* @__PURE__ */ xt(e);
    let c = !1, u = !1;
    l && (c = !/* @__PURE__ */ Me(e), u = /* @__PURE__ */ et(e), e = Wn(e)), r = new Array(e.length);
    for (let f = 0, h = e.length; f < h; f++)
      r[f] = t(
        c ? u ? vt(Re(e[f])) : Re(e[f]) : e[f],
        f,
        void 0,
        o
      );
  } else if (typeof e == "number")
    if (!Number.isInteger(e) || e < 0)
      r = [];
    else {
      r = new Array(e);
      for (let l = 0; l < e; l++)
        r[l] = t(l + 1, l, void 0, o);
    }
  else if (Q(e))
    if (e[Symbol.iterator])
      r = Array.from(
        e,
        (l, c) => t(l, c, void 0, o)
      );
    else {
      const l = Object.keys(e);
      r = new Array(l.length);
      for (let c = 0, u = l.length; c < u; c++) {
        const f = l[c];
        r[c] = t(e[f], f, c, o);
      }
    }
  else
    r = [];
  return r;
}
function Eo(e, t, n, s, r, o) {
  if (n == null && (n = {}), ve.ce || ve.parent && zt(ve.parent) && ve.parent.ce) {
    const u = n, f = Object.keys(u).length > 0;
    return le(), dn(
      fe,
      null,
      [oe("slot", u, s)],
      f ? -2 : 64
    );
  }
  let i = e[t];
  i && i._c && (i._d = !1);
  const l = dt.length;
  le();
  let c;
  try {
    const u = i && Po(i(n)), f = n.key || o || // slot content array of a dynamic conditional slot may have a branch
    // key attached in the `createSlots` helper, respect that
    u && u.key;
    c = dn(
      fe,
      {
        key: (f && !De(f) ? f : `_${t}`) + // #7256 force differentiate fallback content from actual content
        (!u && s ? "_fb" : "")
      },
      u || (s ? s() : []),
      u && e._ === 1 ? 64 : -2
    );
  } catch (u) {
    for (let f = dt.length; f > l; f--) Us();
    throw u;
  } finally {
    i && i._c && (i._d = !0);
  }
  return c.scopeId && (c.slotScopeIds = [c.scopeId + "-s"]), c;
}
function Po(e) {
  return e.some((t) => Ht(t) ? !(t.type === Le || t.type === fe && !Po(t.children)) : !0) ? e : null;
}
const xs = (e) => e ? Yo(e) ? Gn(e) : xs(e.parent) : null, rn = (
  // Move PURE marker to new line to workaround compiler discarding it
  // due to type annotation
  /* @__PURE__ */ he(/* @__PURE__ */ Object.create(null), {
    $: (e) => e,
    $el: (e) => e.vnode.el,
    $data: (e) => e.data,
    $props: (e) => e.props,
    $attrs: (e) => e.attrs,
    $slots: (e) => e.slots,
    $refs: (e) => e.refs,
    $parent: (e) => xs(e.parent),
    $root: (e) => xs(e.root),
    $host: (e) => e.ce,
    $emit: (e) => e.emit,
    $options: (e) => Io(e),
    $forceUpdate: (e) => e.f || (e.f = () => {
      Ds(e.update);
    }),
    $nextTick: (e) => e.n || (e.n = il.bind(e.proxy)),
    $watch: (e) => pl.bind(e)
  })
), rs = (e, t) => e !== ne && !e.__isScriptSetup && J(e, t), Pl = {
  get({ _: e }, t) {
    if (t === "__v_skip")
      return !0;
    const { ctx: n, setupState: s, data: r, props: o, accessCache: i, type: l, appContext: c } = e;
    if (t[0] !== "$") {
      const b = i[t];
      if (b !== void 0)
        switch (b) {
          case 1:
            return s[t];
          case 2:
            return r[t];
          case 4:
            return n[t];
          case 3:
            return o[t];
        }
      else {
        if (rs(s, t))
          return i[t] = 1, s[t];
        if (r !== ne && J(r, t))
          return i[t] = 2, r[t];
        if (J(o, t))
          return i[t] = 3, o[t];
        if (n !== ne && J(n, t))
          return i[t] = 4, n[t];
        vs && (i[t] = 0);
      }
    }
    const u = rn[t];
    let f, h;
    if (u)
      return t === "$attrs" && xe(e.attrs, "get", ""), u(e);
    if (
      // css module (injected by vue-loader)
      (f = l.__cssModules) && (f = f[t])
    )
      return f;
    if (n !== ne && J(n, t))
      return i[t] = 4, n[t];
    if (
      // global properties
      h = c.config.globalProperties, J(h, t)
    )
      return h[t];
  },
  set({ _: e }, t, n) {
    const { data: s, setupState: r, ctx: o } = e;
    return rs(r, t) ? (r[t] = n, !0) : s !== ne && J(s, t) ? (s[t] = n, !0) : J(e.props, t) || t[0] === "$" && t.slice(1) in e ? !1 : (o[t] = n, !0);
  },
  has({
    _: { data: e, setupState: t, accessCache: n, ctx: s, appContext: r, props: o, type: i }
  }, l) {
    let c;
    return !!(n[l] || e !== ne && l[0] !== "$" && J(e, l) || rs(t, l) || J(o, l) || J(s, l) || J(rn, l) || J(r.config.globalProperties, l) || (c = i.__cssModules) && c[l]);
  },
  defineProperty(e, t, n) {
    return n.get != null ? e._.accessCache[t] = 0 : J(n, "value") && this.set(e, t, n.value, null), Reflect.defineProperty(e, t, n);
  }
};
function lr(e) {
  return F(e) ? e.reduce(
    (t, n) => (t[n] = null, t),
    {}
  ) : e;
}
let vs = !0;
function Ol(e) {
  const t = Io(e), n = e.proxy, s = e.ctx;
  vs = !1, t.beforeCreate && cr(t.beforeCreate, e, "bc");
  const {
    // state
    data: r,
    computed: o,
    methods: i,
    watch: l,
    provide: c,
    inject: u,
    // lifecycle
    created: f,
    beforeMount: h,
    mounted: b,
    beforeUpdate: w,
    updated: L,
    activated: C,
    deactivated: $,
    beforeDestroy: D,
    beforeUnmount: j,
    destroyed: W,
    unmounted: O,
    render: B,
    renderTracked: N,
    renderTriggered: G,
    errorCaptured: A,
    serverPrefetch: v,
    // public API
    expose: se,
    inheritAttrs: U,
    // assets
    components: Ae,
    directives: tt,
    filters: wt
  } = t;
  if (u && Il(u, s, null), i)
    for (const q in i) {
      const Y = i[q];
      H(Y) && (s[q] = Y.bind(n));
    }
  if (r) {
    const q = r.call(n, n);
    Q(q) && (e.data = /* @__PURE__ */ Ns(q));
  }
  if (vs = !0, o)
    for (const q in o) {
      const Y = o[q], je = H(Y) ? Y.bind(n, n) : H(Y.get) ? Y.get.bind(n, n) : Xe, M = !H(Y) && H(Y.set) ? Y.set.bind(n) : Xe, We = Ft({
        get: je,
        set: M
      });
      Object.defineProperty(s, q, {
        enumerable: !0,
        configurable: !0,
        get: () => We.value,
        set: (Te) => We.value = Te
      });
    }
  if (l)
    for (const q in l)
      Oo(l[q], s, n, q);
  if (c) {
    const q = H(c) ? c.call(n) : c;
    Reflect.ownKeys(q).forEach((Y) => {
      fl(Y, q[Y]);
    });
  }
  f && cr(f, e, "c");
  function ee(q, Y) {
    F(Y) ? Y.forEach((je) => q(je.bind(n))) : Y && q(Y.bind(n));
  }
  if (ee(vl, h), ee(Ao, b), ee(_l, w), ee(wl, L), ee(bl, C), ee(yl, $), ee(Tl, A), ee(Al, N), ee(Cl, G), ee(To, j), ee(kl, O), ee(Sl, v), F(se))
    if (se.length) {
      const q = e.exposed || (e.exposed = {});
      se.forEach((Y) => {
        Object.defineProperty(q, Y, {
          get: () => n[Y],
          set: (je) => n[Y] = je,
          enumerable: !0
        });
      });
    } else e.exposed || (e.exposed = {});
  B && e.render === Xe && (e.render = B), U != null && (e.inheritAttrs = U), Ae && (e.components = Ae), tt && (e.directives = tt), v && So(e);
}
function Il(e, t, n = Xe) {
  F(e) && (e = _s(e));
  for (const s in e) {
    const r = e[s];
    let o;
    Q(r) ? "default" in r ? o = tn(
      r.from || s,
      r.default,
      !0
    ) : o = tn(r.from || s) : o = tn(r), /* @__PURE__ */ _e(o) ? Object.defineProperty(t, s, {
      enumerable: !0,
      configurable: !0,
      get: () => o.value,
      set: (i) => o.value = i
    }) : t[s] = o;
  }
}
function cr(e, t, n) {
  He(
    F(e) ? e.map((s) => s.bind(t.proxy)) : e.bind(t.proxy),
    t,
    n
  );
}
function Oo(e, t, n, s) {
  let r = s.includes(".") ? wo(n, s) : () => n[s];
  if (ce(e)) {
    const o = t[e];
    H(o) && ns(r, o);
  } else if (H(e))
    ns(r, e.bind(n));
  else if (Q(e))
    if (F(e))
      e.forEach((o) => Oo(o, t, n, s));
    else {
      const o = H(e.handler) ? e.handler.bind(n) : t[e.handler];
      H(o) && ns(r, o, e);
    }
}
function Io(e) {
  const t = e.type, { mixins: n, extends: s } = t, {
    mixins: r,
    optionsCache: o,
    config: { optionMergeStrategies: i }
  } = e.appContext, l = o.get(t);
  let c;
  return l ? c = l : !r.length && !n && !s ? c = t : (c = {}, r.length && r.forEach(
    (u) => In(c, u, i, !0)
  ), In(c, t, i)), Q(t) && o.set(t, c), c;
}
function In(e, t, n, s = !1) {
  const { mixins: r, extends: o } = t;
  o && In(e, o, n, !0), r && r.forEach(
    (i) => In(e, i, n, !0)
  );
  for (const i in t)
    if (!(s && i === "expose")) {
      const l = Ml[i] || n && n[i];
      e[i] = l ? l(e[i], t[i]) : t[i];
    }
  return e;
}
const Ml = {
  data: ar,
  props: ur,
  emits: ur,
  // objects
  methods: Yt,
  computed: Yt,
  // lifecycle
  beforeCreate: we,
  created: we,
  beforeMount: we,
  mounted: we,
  beforeUpdate: we,
  updated: we,
  beforeDestroy: we,
  beforeUnmount: we,
  destroyed: we,
  unmounted: we,
  activated: we,
  deactivated: we,
  errorCaptured: we,
  serverPrefetch: we,
  // assets
  components: Yt,
  directives: Yt,
  // watch
  watch: Ll,
  // provide / inject
  provide: ar,
  inject: Rl
};
function ar(e, t) {
  return t ? e ? function() {
    return he(
      H(e) ? e.call(this, this) : e,
      H(t) ? t.call(this, this) : t
    );
  } : t : e;
}
function Rl(e, t) {
  return Yt(_s(e), _s(t));
}
function _s(e) {
  if (F(e)) {
    const t = {};
    for (let n = 0; n < e.length; n++)
      t[e[n]] = e[n];
    return t;
  }
  return e;
}
function we(e, t) {
  return e ? [...new Set([].concat(e, t))] : t;
}
function Yt(e, t) {
  return e ? he(/* @__PURE__ */ Object.create(null), e, t) : t;
}
function ur(e, t) {
  return e ? F(e) && F(t) ? [.../* @__PURE__ */ new Set([...e, ...t])] : he(
    /* @__PURE__ */ Object.create(null),
    lr(e),
    lr(t ?? {})
  ) : t;
}
function Ll(e, t) {
  if (!e) return t;
  if (!t) return e;
  const n = he(/* @__PURE__ */ Object.create(null), e);
  for (const s in t)
    n[s] = we(e[s], t[s]);
  return n;
}
function Mo() {
  return {
    app: null,
    config: {
      isNativeTag: Br,
      performance: !1,
      globalProperties: {},
      optionMergeStrategies: {},
      errorHandler: void 0,
      warnHandler: void 0,
      compilerOptions: {}
    },
    mixins: [],
    components: {},
    directives: {},
    provides: /* @__PURE__ */ Object.create(null),
    optionsCache: /* @__PURE__ */ new WeakMap(),
    propsCache: /* @__PURE__ */ new WeakMap(),
    emitsCache: /* @__PURE__ */ new WeakMap()
  };
}
let jl = 0;
function $l(e, t) {
  return function(s, r = null) {
    H(s) || (s = he({}, s)), r != null && !Q(r) && (r = null);
    const o = Mo(), i = /* @__PURE__ */ new WeakSet(), l = [];
    let c = !1;
    const u = o.app = {
      _uid: jl++,
      _component: s,
      _props: r,
      _container: null,
      _context: o,
      _instance: null,
      version: pc,
      get config() {
        return o.config;
      },
      set config(f) {
      },
      use(f, ...h) {
        return i.has(f) || (f && H(f.install) ? (i.add(f), f.install(u, ...h)) : H(f) && (i.add(f), f(u, ...h))), u;
      },
      mixin(f) {
        return o.mixins.includes(f) || o.mixins.push(f), u;
      },
      component(f, h) {
        return h ? (o.components[f] = h, u) : o.components[f];
      },
      directive(f, h) {
        return h ? (o.directives[f] = h, u) : o.directives[f];
      },
      mount(f, h, b) {
        if (!c) {
          const w = u._ceVNode || oe(s, r);
          return w.appContext = o, b === !0 ? b = "svg" : b === !1 && (b = void 0), e(w, f, b), c = !0, u._container = f, f.__vue_app__ = u, Gn(w.component);
        }
      },
      onUnmount(f) {
        l.push(f);
      },
      unmount() {
        c && (He(
          l,
          u._instance,
          16
        ), e(null, u._container), delete u._container.__vue_app__);
      },
      provide(f, h) {
        return o.provides[f] = h, u;
      },
      runWithContext(f) {
        const h = Dt;
        Dt = u;
        try {
          return f();
        } finally {
          Dt = h;
        }
      }
    };
    return u;
  };
}
let Dt = null;
const Nl = (e, t) => t === "modelValue" || t === "model-value" ? e.modelModifiers : e[`${t}Modifiers`] || e[`${Fe(t)}Modifiers`] || e[`${Ot(t)}Modifiers`];
function Fl(e, t, ...n) {
  if (e.isUnmounted) return;
  const s = e.vnode.props || ne;
  let r = n;
  const o = t.startsWith("update:"), i = o && Nl(s, t.slice(7));
  i && (i.trim && (r = n.map((f) => ce(f) ? f.trim() : f)), i.number && (r = r.map(ki)));
  let l, c = s[l = Jn(t)] || // also try camelCase event handler (#2249)
  s[l = Jn(Fe(t))];
  !c && o && (c = s[l = Jn(Ot(t))]), c && He(
    c,
    e,
    6,
    r
  );
  const u = s[l + "Once"];
  if (u) {
    if (!e.emitted)
      e.emitted = {};
    else if (e.emitted[l])
      return;
    e.emitted[l] = !0, He(
      u,
      e,
      6,
      r
    );
  }
}
const zl = /* @__PURE__ */ new WeakMap();
function Ro(e, t, n = !1) {
  const s = n ? zl : t.emitsCache, r = s.get(e);
  if (r !== void 0)
    return r;
  const o = e.emits;
  let i = {}, l = !1;
  if (!H(e)) {
    const c = (u) => {
      const f = Ro(u, t, !0);
      f && (l = !0, he(i, f));
    };
    !n && t.mixins.length && t.mixins.forEach(c), e.extends && c(e.extends), e.mixins && e.mixins.forEach(c);
  }
  return !o && !l ? (Q(e) && s.set(e, null), null) : (F(o) ? o.forEach((c) => i[c] = null) : he(i, o), Q(e) && s.set(e, i), i);
}
function Un(e, t) {
  return !e || !Nn(t) ? !1 : (t = t.slice(2), t = t === "Once" ? t : t.replace(/Once$/, ""), J(e, t[0].toLowerCase() + t.slice(1)) || J(e, Ot(t)) || J(e, t));
}
function fr(e) {
  const {
    type: t,
    vnode: n,
    proxy: s,
    withProxy: r,
    propsOptions: [o],
    slots: i,
    attrs: l,
    emit: c,
    render: u,
    renderCache: f,
    props: h,
    data: b,
    setupState: w,
    ctx: L,
    inheritAttrs: C
  } = e, $ = Pn(e);
  let D, j;
  try {
    if (n.shapeFlag & 4) {
      const O = r || s, B = O;
      D = Je(
        u.call(
          B,
          O,
          f,
          h,
          w,
          b,
          L
        )
      ), j = l;
    } else {
      const O = t;
      D = Je(
        O.length > 1 ? O(
          h,
          { attrs: l, slots: i, emit: c }
        ) : O(
          h,
          null
        )
      ), j = t.props ? l : Dl(l);
    }
  } catch (O) {
    dt.length = 0, Bn(O, e, 1), D = oe(Le);
  }
  let W = D;
  if (j && C !== !1) {
    const O = Object.keys(j), { shapeFlag: B } = W;
    O.length && B & 7 && (o && O.some(Fn) && (j = Hl(
      j,
      o
    )), W = Wt(W, j, !1, !0));
  }
  if (n.dirs && (W = Wt(W, null, !1, !0), W.dirs = W.dirs ? W.dirs.concat(n.dirs) : n.dirs), n.transition) {
    const O = Vn(W.type) && ko(W) || W;
    nn(O, n.transition);
  }
  return D = W, Pn($), D;
}
function Lo(e, t = !0) {
  let n;
  for (let s = 0; s < e.length; s++) {
    const r = e[s];
    if (Ht(r)) {
      if (r.type !== Le || r.children === "v-if") {
        if (n)
          return;
        n = r;
      }
    } else
      return;
  }
  return n;
}
const Dl = (e) => {
  let t;
  for (const n in e)
    (n === "class" || n === "style" || Nn(n)) && ((t || (t = {}))[n] = e[n]);
  return t;
}, Hl = (e, t) => {
  const n = {};
  for (const s in e)
    (!Fn(s) || !(s.slice(9) in t)) && (n[s] = e[s]);
  return n;
};
function Wl(e, t, n) {
  const { props: s, children: r, component: o } = e, { props: i, children: l, patchFlag: c } = t, u = o.emitsOptions;
  if (t.dirs || t.transition)
    return !0;
  if (n && c >= 0) {
    if (c & 1024)
      return !0;
    if (c & 16)
      return s ? dr(s, i, u) : !!i;
    if (c & 8) {
      const f = t.dynamicProps;
      for (let h = 0; h < f.length; h++) {
        const b = f[h];
        if (jo(i, s, b) && !Un(u, b))
          return !0;
      }
    }
  } else
    return (r || l) && (!l || !l.$stable) ? !0 : s === i ? !1 : s ? i ? dr(s, i, u) : !0 : !!i;
  return !1;
}
function dr(e, t, n) {
  const s = Object.keys(t);
  if (s.length !== Object.keys(e).length)
    return !0;
  for (let r = 0; r < s.length; r++) {
    const o = s[r];
    if (!J(e, o) || jo(t, e, o) && !Un(n, o))
      return !0;
  }
  return !1;
}
function jo(e, t, n) {
  const s = e[n], r = t[n];
  return n === "style" && Q(s) && Q(r) ? !Hn(s, r) : s !== r;
}
function Bl({ vnode: e, parent: t, suspense: n }, s) {
  for (; t; ) {
    const r = t.subTree;
    if (r.suspense && r.suspense.activeBranch === e && (r.suspense.vnode.el = r.el = s, e = r), r === e)
      (e = t.vnode).el = s, t = t.parent;
    else
      break;
  }
  n && n.activeBranch === e && (n.vnode.el = s);
}
const $o = {}, No = () => Object.create($o), Fo = (e) => Object.getPrototypeOf(e) === $o;
function Vl(e, t, n, s = !1) {
  const r = {}, o = No();
  e.propsDefaults = /* @__PURE__ */ Object.create(null), zo(e, t, r, o);
  for (const i in e.propsOptions[0])
    i in r || (r[i] = void 0);
  n ? e.props = s ? r : /* @__PURE__ */ Yi(r) : e.type.props ? e.props = r : e.props = o, e.attrs = o;
}
function Ul(e, t, n, s) {
  const {
    props: r,
    attrs: o,
    vnode: { patchFlag: i }
  } = e, l = /* @__PURE__ */ X(r), [c] = e.propsOptions;
  let u = !1;
  if (
    // always force full diff in dev
    // - #1942 if hmr is enabled with sfc component
    // - vite#872 non-sfc component used by sfc component
    (s || i > 0) && !(i & 16)
  ) {
    if (i & 8) {
      const f = e.vnode.dynamicProps;
      for (let h = 0; h < f.length; h++) {
        let b = f[h];
        if (Un(e.emitsOptions, b))
          continue;
        const w = t[b];
        if (c)
          if (J(o, b))
            w !== o[b] && (o[b] = w, u = !0);
          else {
            const L = Fe(b);
            r[L] = ws(
              c,
              l,
              L,
              w,
              e,
              !1
            );
          }
        else
          w !== o[b] && (o[b] = w, u = !0);
      }
    }
  } else {
    zo(e, t, r, o) && (u = !0);
    let f;
    for (const h in l)
      (!t || // for camelCase
      !J(t, h) && // it's possible the original props was passed in as kebab-case
      // and converted to camelCase (#955)
      ((f = Ot(h)) === h || !J(t, f))) && (c ? n && // for camelCase
      (n[h] !== void 0 || // for kebab-case
      n[f] !== void 0) && (r[h] = ws(
        c,
        l,
        h,
        void 0,
        e,
        !0
      )) : delete r[h]);
    if (o !== l)
      for (const h in o)
        (!t || !J(t, h)) && (delete o[h], u = !0);
  }
  u && ut(e.attrs, "set", "");
}
function zo(e, t, n, s) {
  const [r, o] = e.propsOptions;
  let i = !1, l;
  if (t)
    for (let c in t) {
      if (Qt(c))
        continue;
      const u = t[c];
      let f;
      r && J(r, f = Fe(c)) ? !o || !o.includes(f) ? n[f] = u : (l || (l = {}))[f] = u : Un(e.emitsOptions, c) || (!(c in s) || u !== s[c]) && (s[c] = u, i = !0);
    }
  if (o) {
    const c = /* @__PURE__ */ X(n), u = l || ne;
    for (let f = 0; f < o.length; f++) {
      const h = o[f];
      n[h] = ws(
        r,
        c,
        h,
        u[h],
        e,
        !J(u, h)
      );
    }
  }
  return i;
}
function ws(e, t, n, s, r, o) {
  const i = e[n];
  if (i != null) {
    const l = J(i, "default");
    if (l && s === void 0) {
      const c = i.default;
      if (i.type !== Function && !i.skipFactory && H(c)) {
        const { propsDefaults: u } = r;
        if (n in u)
          s = u[n];
        else {
          const f = bn(r);
          s = u[n] = c.call(
            null,
            t
          ), f();
        }
      } else
        s = c;
      r.ce && r.ce._setProp(n, s);
    }
    i[
      0
      /* shouldCast */
    ] && (o && !l ? s = !1 : i[
      1
      /* shouldCastTrue */
    ] && (s === "" || s === Ot(n)) && (s = !0));
  }
  return s;
}
const Kl = /* @__PURE__ */ new WeakMap();
function Do(e, t, n = !1) {
  const s = n ? Kl : t.propsCache, r = s.get(e);
  if (r)
    return r;
  const o = e.props, i = {}, l = [];
  let c = !1;
  if (!H(e)) {
    const f = (h) => {
      c = !0;
      const [b, w] = Do(h, t, !0);
      he(i, b), w && l.push(...w);
    };
    !n && t.mixins.length && t.mixins.forEach(f), e.extends && f(e.extends), e.mixins && e.mixins.forEach(f);
  }
  if (!o && !c)
    return Q(e) && s.set(e, Et), Et;
  if (F(o))
    for (let f = 0; f < o.length; f++) {
      const h = Fe(o[f]);
      hr(h) && (i[h] = ne);
    }
  else if (o)
    for (const f in o) {
      const h = Fe(f);
      if (hr(h)) {
        const b = o[f], w = i[h] = F(b) || H(b) ? { type: b } : he({}, b), L = w.type;
        let C = !1, $ = !0;
        if (F(L))
          for (let D = 0; D < L.length; ++D) {
            const j = L[D], W = H(j) && j.name;
            if (W === "Boolean") {
              C = !0;
              break;
            } else W === "String" && ($ = !1);
          }
        else
          C = H(L) && L.name === "Boolean";
        w[
          0
          /* shouldCast */
        ] = C, w[
          1
          /* shouldCastTrue */
        ] = $, (C || J(w, "default")) && l.push(h);
      }
    }
  const u = [i, l];
  return Q(e) && s.set(e, u), u;
}
function hr(e) {
  return e[0] !== "$" && !Qt(e);
}
const Bs = (e) => e === "_" || e === "_ctx" || e === "$stable", Vs = (e) => F(e) ? e.map(Je) : [Je(e)], Gl = (e, t, n) => {
  if (t._n)
    return t;
  const s = Nt((...r) => Vs(t(...r)), n);
  return s._c = !1, s;
}, Ho = (e, t, n) => {
  const s = e._ctx;
  for (const r in e) {
    if (Bs(r)) continue;
    const o = e[r];
    if (H(o))
      t[r] = Gl(r, o, s);
    else if (o != null) {
      const i = Vs(o);
      t[r] = () => i;
    }
  }
}, Wo = (e, t) => {
  const n = Vs(t);
  e.slots.default = () => n;
}, Bo = (e, t, n) => {
  for (const s in t)
    (n || !Bs(s)) && (e[s] = t[s]);
}, ql = (e, t, n) => {
  const s = e.slots = No();
  if (e.vnode.shapeFlag & 32) {
    const r = t._;
    r ? (Bo(s, t, n), n && qr(s, "_", r, !0)) : Ho(t, s);
  } else t && Wo(e, t);
}, Jl = (e, t, n) => {
  const { vnode: s, slots: r } = e;
  let o = !0, i = ne;
  if (s.shapeFlag & 32) {
    const l = t._;
    l ? n && l === 1 ? o = !1 : Bo(r, t, n) : (o = !t.$stable, Ho(t, r)), i = t;
  } else t && (Wo(e, t), i = { default: 1 });
  if (o)
    for (const l in r)
      !Bs(l) && i[l] == null && delete r[l];
}, ke = tc;
function Yl(e) {
  return Xl(e);
}
function Xl(e, t) {
  const n = Dn();
  n.__VUE__ = !0;
  const {
    insert: s,
    remove: r,
    patchProp: o,
    createElement: i,
    createText: l,
    createComment: c,
    setText: u,
    setElementText: f,
    parentNode: h,
    nextSibling: b,
    setScopeId: w = Xe,
    insertStaticContent: L
  } = e, C = (a, d, p, x = null, g = null, y = null, S = void 0, k = null, _ = !!d.dynamicChildren) => {
    if (a === d)
      return;
    a && !Gt(a, d) && (x = st(a), Te(a, g, y, !0), a = null), d.patchFlag === -2 && (_ = !1, d.dynamicChildren = null), d.dynamicChildren && a && a.dynamicChildren && a.dynamicChildren.hasOnce && (d.dynamicChildren === Et && (d.dynamicChildren = []), d.dynamicChildren.hasOnce = !0);
    const { type: m, ref: I, shapeFlag: T } = d;
    switch (m) {
      case Kn:
        $(a, d, p, x);
        break;
      case Le:
        D(a, d, p, x);
        break;
      case is:
        a == null && j(d, p, x, S);
        break;
      case fe:
        Ae(
          a,
          d,
          p,
          x,
          g,
          y,
          S,
          k,
          _
        );
        break;
      default:
        T & 1 ? B(
          a,
          d,
          p,
          x,
          g,
          y,
          S,
          k,
          _
        ) : T & 6 ? tt(
          a,
          d,
          p,
          x,
          g,
          y,
          S,
          k,
          _
        ) : (T & 64 || T & 128) && m.process(
          a,
          d,
          p,
          x,
          g,
          y,
          S,
          k,
          _,
          ie
        );
    }
    I != null && g ? sn(I, a && a.ref, y, d || a, !d) : I == null && a && a.ref != null && sn(a.ref, null, y, a, !0);
  }, $ = (a, d, p, x) => {
    if (a == null)
      s(
        d.el = l(d.children),
        p,
        x
      );
    else {
      const g = d.el = a.el;
      d.children !== a.children && u(g, d.children);
    }
  }, D = (a, d, p, x) => {
    a == null ? s(
      d.el = c(d.children || ""),
      p,
      x
    ) : d.el = a.el;
  }, j = (a, d, p, x) => {
    [a.el, a.anchor] = L(
      a.children,
      d,
      p,
      x,
      a.el,
      a.anchor
    );
  }, W = ({ el: a, anchor: d }, p, x) => {
    let g;
    for (; a && a !== d; )
      g = b(a), s(a, p, x), a = g;
    s(d, p, x);
  }, O = ({ el: a, anchor: d }) => {
    let p;
    for (; a && a !== d; )
      p = b(a), r(a), a = p;
    r(d);
  }, B = (a, d, p, x, g, y, S, k, _) => {
    if (d.type === "svg" ? S = "svg" : d.type === "math" && (S = "mathml"), a == null)
      N(
        d,
        p,
        x,
        g,
        y,
        S,
        k,
        _
      );
    else {
      const m = a.el && a.el._isVueCE ? a.el : null;
      try {
        m && m._beginPatch(), v(
          a,
          d,
          g,
          y,
          S,
          k,
          _
        );
      } finally {
        m && m._endPatch();
      }
    }
  }, N = (a, d, p, x, g, y, S, k) => {
    let _, m;
    const { props: I, shapeFlag: T, transition: R, dirs: z } = a;
    if (_ = a.el = i(
      a.type,
      y,
      I && I.is,
      I
    ), T & 8 ? f(_, a.children) : T & 16 && A(
      a.children,
      _,
      null,
      x,
      g,
      os(a, y),
      S,
      k
    ), z && kt(a, null, x, "created"), G(_, a, a.scopeId, S, x), I) {
      for (const te in I)
        te !== "value" && !Qt(te) && o(_, te, null, I[te], y, x);
      "value" in I && o(_, "value", null, I.value, y), (m = I.onVnodeBeforeMount) && Ue(m, x, a);
    }
    z && kt(a, null, x, "beforeMount");
    const K = Ql(g, R);
    K ? R.beforeEnter(_) : R && !R.persisted && (_[ec] = !0), s(_, d, p), ((m = I && I.onVnodeMounted) || K || z) && ke(() => {
      m && Ue(m, x, a), K && R.enter(_), z && kt(a, null, x, "mounted");
    }, g);
  }, G = (a, d, p, x, g) => {
    if (p && w(a, p), x)
      for (let y = 0; y < x.length; y++)
        w(a, x[y]);
    if (g) {
      let y = g.subTree;
      if (d === y || Ko(y.type) && (y.ssContent === d || y.ssFallback === d)) {
        const S = g.vnode;
        G(
          a,
          S,
          S.scopeId,
          S.slotScopeIds,
          g.parent
        );
      }
    }
  }, A = (a, d, p, x, g, y, S, k, _ = 0) => {
    for (let m = _; m < a.length; m++) {
      const I = a[m] = k ? at(a[m]) : Je(a[m]);
      C(
        null,
        I,
        d,
        p,
        x,
        g,
        y,
        S,
        k
      );
    }
  }, v = (a, d, p, x, g, y, S) => {
    const k = d.el = a.el;
    let { patchFlag: _, dynamicChildren: m, dirs: I } = d;
    _ |= a.patchFlag & 16;
    const T = a.props || ne, R = d.props || ne;
    let z;
    if (p && St(p, !1), (z = R.onVnodeBeforeUpdate) && Ue(z, p, d, a), I && kt(d, a, p, "beforeUpdate"), p && St(p, !0), // #6385 the old vnode may be a user-wrapped non-isomorphic block
    // Force full diff when block metadata is unstable.
    m && (!a.dynamicChildren || a.dynamicChildren.length !== m.length) && (_ = 0, S = !1, m = null), (T.innerHTML && R.innerHTML == null || T.textContent && R.textContent == null) && f(k, ""), m ? se(
      a.dynamicChildren,
      m,
      k,
      p,
      x,
      os(d, g),
      y
    ) : S || Y(
      a,
      d,
      k,
      null,
      p,
      x,
      os(d, g),
      y,
      !1
    ), _ > 0) {
      if (_ & 16)
        U(k, T, R, p, g);
      else if (_ & 2 && T.class !== R.class && o(k, "class", null, R.class, g), _ & 4 && o(k, "style", T.style, R.style, g), _ & 8) {
        const K = d.dynamicProps;
        for (let te = 0; te < K.length; te++) {
          const Z = K[te], ue = T[Z], me = R[Z];
          (me !== ue || Z === "value") && o(k, Z, ue, me, g, p);
        }
      }
      _ & 1 && a.children !== d.children && f(k, d.children);
    } else !S && m == null && U(k, T, R, p, g);
    ((z = R.onVnodeUpdated) || I) && ke(() => {
      z && Ue(z, p, d, a), I && kt(d, a, p, "updated");
    }, x);
  }, se = (a, d, p, x, g, y, S) => {
    for (let k = 0; k < d.length; k++) {
      const _ = a[k], m = d[k], I = _.el || on(_), T = (
        // oldVNode may be an errored async setup() component inside Suspense
        // which will not have a mounted element
        I && // - In the case of a Fragment, we need to provide the actual parent
        // of the Fragment itself so it can move its children.
        (_.type === fe || // - In the case of different nodes, there is going to be a replacement
        // which also requires the correct parent container
        !Gt(_, m) || // - In the case of a component, it could contain anything.
        _.shapeFlag & 198) ? h(I) : (
          // In other cases, the parent container is not actually used so we
          // just pass the block element here to avoid a DOM parentNode call.
          p
        )
      );
      C(
        _,
        m,
        T,
        null,
        x,
        g,
        y,
        S,
        !0
      );
    }
  }, U = (a, d, p, x, g) => {
    if (d !== p) {
      if (d !== ne)
        for (const y in d)
          !Qt(y) && !(y in p) && o(
            a,
            y,
            d[y],
            null,
            g,
            x
          );
      for (const y in p) {
        if (Qt(y)) continue;
        const S = p[y], k = d[y];
        S !== k && y !== "value" && o(a, y, k, S, g, x);
      }
      "value" in p && o(a, "value", d.value, p.value, g);
    }
  }, Ae = (a, d, p, x, g, y, S, k, _) => {
    const m = d.el = a ? a.el : l(""), I = d.anchor = a ? a.anchor : l("");
    let { patchFlag: T, dynamicChildren: R, slotScopeIds: z } = d;
    z && (k = k ? k.concat(z) : z), a == null ? (s(m, p, x), s(I, p, x), A(
      // #10007
      // such fragment like `<></>` will be compiled into
      // a fragment which doesn't have a children.
      // In this case fallback to an empty array
      d.children || [],
      p,
      I,
      g,
      y,
      S,
      k,
      _
    )) : T > 0 && T & 64 && R && // #2715 the previous fragment could've been a BAILed one as a result
    // of renderSlot() with no valid children
    a.dynamicChildren && a.dynamicChildren.length === R.length ? (se(
      a.dynamicChildren,
      R,
      p,
      g,
      y,
      S,
      k
    ), // #2080 if the stable fragment has a key, it's a <template v-for> that may
    //  get moved around. Make sure all root level vnodes inherit el.
    // #2134 or if it's a component root, it may also get moved around
    // as the component is being moved.
    (d.key != null || g && d === g.subTree) && Vo(
      a,
      d,
      !0
      /* shallow */
    )) : Y(
      a,
      d,
      p,
      I,
      g,
      y,
      S,
      k,
      _
    );
  }, tt = (a, d, p, x, g, y, S, k, _) => {
    d.slotScopeIds = k, a == null ? d.shapeFlag & 512 ? g.ctx.activate(
      d,
      p,
      x,
      S,
      _
    ) : wt(
      d,
      p,
      x,
      g,
      y,
      S,
      _
    ) : nt(a, d, _);
  }, wt = (a, d, p, x, g, y, S) => {
    const k = a.component = lc(
      a,
      x,
      g
    );
    if (Ws(a) && (k.ctx.renderer = ie), ac(k, !1, S), k.asyncDep) {
      if (g && g.registerDep(
        k,
        ee,
        S,
        y
      ), !a.el) {
        const _ = k.subTree = oe(Le);
        D(null, _, d, p), a.placeholder = _.el;
      }
    } else
      ee(
        k,
        a,
        d,
        p,
        g,
        y,
        S
      );
  }, nt = (a, d, p) => {
    const x = d.component = a.component;
    if (Wl(a, d, p))
      if (x.asyncDep && !x.asyncResolved) {
        d.el = a.el, d.placeholder = a.placeholder, q(x, d, p);
        return;
      } else
        x.next = d, x.update();
    else
      d.el = a.el, d.placeholder = a.placeholder, x.vnode = d;
  }, ee = (a, d, p, x, g, y, S) => {
    const k = () => {
      if (a.isMounted) {
        let { next: T, bu: R, u: z, parent: K, vnode: te } = a;
        {
          const Be = Uo(a);
          if (Be) {
            T && (T.el = te.el, q(a, T, S)), Be.asyncDep.then(() => {
              const rt = () => {
                a.job.flags & 8 || (g && g.pendingBranch ? ke(rt, g) : m());
              };
              rt.flags = 48, ke(rt, g);
            });
            return;
          }
        }
        let Z = T, ue;
        St(a, !1), T ? (T.el = te.el, q(a, T, S)) : T = te, R && Yn(R), (ue = T.props && T.props.onVnodeBeforeUpdate) && Ue(ue, K, T, te), St(a, !0);
        const me = fr(a), $e = a.subTree;
        a.subTree = me, C(
          $e,
          me,
          // parent may have changed if it's in a teleport. a root that is a
          // pending async component only has its placeholder comment
          h(
            $e.el || on($e)
          ),
          // anchor may have changed if it's in a fragment
          st($e),
          a,
          g,
          y
        ), T.el = me.el, Z === null && Bl(a, me.el), z && ke(z, g), (ue = T.props && T.props.onVnodeUpdated) && ke(
          () => Ue(ue, K, T, te),
          g
        );
      } else {
        let T;
        const { el: R, props: z } = d, { bm: K, m: te, parent: Z, root: ue, type: me } = a, $e = zt(d);
        St(a, !1), K && Yn(K), !$e && (T = z && z.onVnodeBeforeMount) && Ue(T, Z, d), St(a, !0);
        {
          ue.ce && ue.ce._hasShadowRoot() && ue.ce._injectChildStyle(
            me,
            a.parent ? a.parent.type : void 0
          );
          const Be = a.subTree = fr(a);
          C(
            null,
            Be,
            p,
            x,
            a,
            g,
            y
          ), d.el = Be.el;
        }
        if (te && ke(te, g), !$e && (T = z && z.onVnodeMounted)) {
          const Be = d;
          ke(
            () => Ue(T, Z, Be),
            g
          );
        }
        (d.shapeFlag & 256 || Z && zt(Z.vnode) && Z.vnode.shapeFlag & 256) && a.a && ke(a.a, g), a.isMounted = !0, d = p = x = null;
      }
    };
    a.scope.on();
    const _ = a.effect = new Qr(k);
    a.scope.off();
    const m = a.update = _.run.bind(_), I = a.job = _.runIfDirty.bind(_);
    I.i = a, I.id = a.uid, _.scheduler = () => Ds(I), St(a, !0), m();
  }, q = (a, d, p) => {
    d.component = a;
    const x = a.vnode.props;
    a.vnode = d, a.next = null, Ul(a, d.props, x, p), Jl(a, d.children, p), Qe(), bo(a), Ze();
  }, Y = (a, d, p, x, g, y, S, k, _ = !1) => {
    const m = a && a.children, I = a ? a.shapeFlag : 0, T = d.children, { patchFlag: R, shapeFlag: z } = d;
    if (R > 0) {
      if (R & 128) {
        M(
          m,
          T,
          p,
          x,
          g,
          y,
          S,
          k,
          _
        );
        return;
      } else if (R & 256) {
        je(
          m,
          T,
          p,
          x,
          g,
          y,
          S,
          k,
          _
        );
        return;
      }
    }
    z & 8 ? (I & 16 && ae(m, g, y), T !== m && f(p, T)) : I & 16 ? z & 16 ? M(
      m,
      T,
      p,
      x,
      g,
      y,
      S,
      k,
      _
    ) : ae(m, g, y, !0) : (I & 8 && f(p, ""), z & 16 && A(
      T,
      p,
      x,
      g,
      y,
      S,
      k,
      _
    ));
  }, je = (a, d, p, x, g, y, S, k, _) => {
    a = a || Et, d = d || Et;
    const m = a.length, I = d.length, T = Math.min(m, I);
    let R;
    for (R = 0; R < T; R++) {
      const z = d[R] = _ ? at(d[R]) : Je(d[R]);
      C(
        a[R],
        z,
        p,
        null,
        g,
        y,
        S,
        k,
        _
      );
    }
    m > I ? ae(
      a,
      g,
      y,
      !0,
      !1,
      T
    ) : A(
      d,
      p,
      x,
      g,
      y,
      S,
      k,
      _,
      T
    );
  }, M = (a, d, p, x, g, y, S, k, _) => {
    let m = 0;
    const I = d.length;
    let T = a.length - 1, R = I - 1;
    for (; m <= T && m <= R; ) {
      const z = a[m], K = d[m] = _ ? at(d[m]) : Je(d[m]);
      if (Gt(z, K))
        C(
          z,
          K,
          p,
          null,
          g,
          y,
          S,
          k,
          _
        );
      else
        break;
      m++;
    }
    for (; m <= T && m <= R; ) {
      const z = a[T], K = d[R] = _ ? at(d[R]) : Je(d[R]);
      if (Gt(z, K))
        C(
          z,
          K,
          p,
          null,
          g,
          y,
          S,
          k,
          _
        );
      else
        break;
      T--, R--;
    }
    if (m > T) {
      if (m <= R) {
        const z = R + 1, K = z < I ? d[z].el : x;
        for (; m <= R; )
          C(
            null,
            d[m] = _ ? at(d[m]) : Je(d[m]),
            p,
            K,
            g,
            y,
            S,
            k,
            _
          ), m++;
      }
    } else if (m > R)
      for (; m <= T; )
        Te(a[m], g, y, !0), m++;
    else {
      const z = m, K = m, te = /* @__PURE__ */ new Map();
      for (m = K; m <= R; m++) {
        const Pe = d[m] = _ ? at(d[m]) : Je(d[m]);
        Pe.key != null && te.set(Pe.key, m);
      }
      let Z, ue = 0;
      const me = R - K + 1;
      let $e = !1, Be = 0;
      const rt = new Array(me);
      for (m = 0; m < me; m++) rt[m] = 0;
      for (m = z; m <= T; m++) {
        const Pe = a[m];
        if (ue >= me) {
          Te(Pe, g, y, !0);
          continue;
        }
        let Ve;
        if (Pe.key != null)
          Ve = te.get(Pe.key);
        else
          for (Z = K; Z <= R; Z++)
            if (rt[Z - K] === 0 && Gt(Pe, d[Z])) {
              Ve = Z;
              break;
            }
        Ve === void 0 ? Te(Pe, g, y, !0) : (rt[Ve - K] = m + 1, Ve >= Be ? Be = Ve : $e = !0, C(
          Pe,
          d[Ve],
          p,
          null,
          g,
          y,
          S,
          k,
          _
        ), ue++);
      }
      const qs = $e ? Zl(rt) : Et;
      for (Z = qs.length - 1, m = me - 1; m >= 0; m--) {
        const Pe = K + m, Ve = d[Pe], Js = d[Pe + 1], Ys = Pe + 1 < I ? (
          // #13559, #14173 fallback to el placeholder for unresolved async component
          Js.el || on(Js)
        ) : x;
        rt[m] === 0 ? C(
          null,
          Ve,
          p,
          Ys,
          g,
          y,
          S,
          k,
          _
        ) : $e && (Z < 0 || m !== qs[Z] ? We(Ve, p, Ys, 2) : Z--);
      }
    }
  }, We = (a, d, p, x, g = null) => {
    const { el: y, type: S, transition: k, children: _, shapeFlag: m } = a;
    if (m & 6) {
      We(a.component.subTree, d, p, x);
      return;
    }
    if (m & 128) {
      a.suspense.move(d, p, x);
      return;
    }
    if (m & 64) {
      S.move(a, d, p, ie);
      return;
    }
    if (S === fe) {
      s(y, d, p);
      for (let T = 0; T < _.length; T++)
        We(_[T], d, p, x);
      s(a.anchor, d, p);
      return;
    }
    if (S === is) {
      W(a, d, p);
      return;
    }
    if (x !== 2 && m & 1 && k)
      if (x === 0)
        k.persisted && !y[ss] ? s(y, d, p) : (k.beforeEnter(y), s(y, d, p), ke(() => k.enter(y), g));
      else {
        const { leave: T, delayLeave: R, afterLeave: z } = k, K = () => {
          a.ctx.isUnmounted ? r(y) : s(y, d, p);
        }, te = () => {
          const Z = y._isLeaving || !!y[ss];
          y._isLeaving && y[ss](
            !0
            /* cancelled */
          ), k.persisted && !Z ? K() : T(y, () => {
            K(), z && z();
          });
        };
        R ? R(y, K, te) : te();
      }
    else
      s(y, d, p);
  }, Te = (a, d, p, x = !1, g = !1) => {
    const {
      type: y,
      props: S,
      ref: k,
      children: _,
      dynamicChildren: m,
      shapeFlag: I,
      patchFlag: T,
      dirs: R,
      cacheIndex: z,
      memo: K
    } = a;
    if ((T === -2 || m && m.hasOnce) && (g = !1), k != null && (Qe(), sn(k, null, p, a, !0), Ze()), z != null && (!a.ctx || a.ctx === d) && (d.renderCache[z] = void 0), I & 256 && d.ctx.deactivate(a))
      return;
    const te = I & 1 && R, Z = !zt(a);
    let ue;
    if (Z && (ue = S && S.onVnodeBeforeUnmount) && Ue(ue, d, a), I & 6)
      ge(a.component, p, x);
    else {
      if (I & 128) {
        a.suspense.unmount(p, x);
        return;
      }
      te && kt(a, null, d, "beforeUnmount"), I & 64 ? a.type.remove(
        a,
        d,
        p,
        ie,
        x
      ) : m && // #5154
      // when v-once is used inside a block, setBlockTracking(-1) marks the
      // parent block with hasOnce: true
      // so that it doesn't take the fast path during unmount - otherwise
      // components nested in v-once are never unmounted.
      !m.hasOnce && // #1153: fast path should not be taken for non-stable (v-for) fragments
      (y !== fe || T > 0 && T & 64) ? ae(
        m,
        d,
        p,
        !1,
        !0
      ) : (y === fe && T & 384 || !g && I & 16) && ae(_, d, p), x && Vt(a);
    }
    const me = K != null && z == null;
    (Z && (ue = S && S.onVnodeUnmounted) || te || me) && ke(() => {
      ue && Ue(ue, d, a), te && kt(a, null, d, "unmounted"), me && (a.el = null);
    }, p);
  }, Vt = (a) => {
    const { type: d, el: p, anchor: x, transition: g } = a, y = d === fe;
    if (y && !g) {
      Mt(p, x);
      return;
    }
    if (d === is) {
      O(a), g && !g.persisted && g.afterLeave && g.afterLeave();
      return;
    }
    const S = y ? ys(a) : a, k = () => {
      y ? h(p) && Mt(p, x) : r(p), g && !g.persisted && g.afterLeave && g.afterLeave();
    };
    if (S && S.shapeFlag & 1 && g && !g.persisted) {
      const { leave: _, delayLeave: m } = g, I = S.el, T = () => _(I, k);
      m ? m(I, k, T) : T();
    } else
      k();
  }, Mt = (a, d) => {
    let p;
    for (; a !== d; )
      p = b(a), r(a), a = p;
    r(d);
  }, ge = (a, d, p) => {
    const { bum: x, scope: g, job: y, subTree: S, um: k, m: _, a: m } = a;
    pr(_), pr(m), x && Yn(x), g.stop(), y ? (y.flags |= 8, Te(S, a, d, p)) : S && (S.transition = a.vnode.transition, Te(S, a, d, p)), k && ke(k, d);
    const I = () => {
      a.isUnmounted = !0;
    };
    I.flags = 16, ke(I, d);
  }, ae = (a, d, p, x = !1, g = !1, y = 0) => {
    for (let S = y; S < a.length; S++)
      Te(a[S], d, p, x, g);
  }, st = (a) => {
    if (a.shapeFlag & 6)
      return st(a.component.subTree);
    if (a.shapeFlag & 128)
      return a.suspense.next();
    const d = b(a.anchor || a.el), p = d && d[gl];
    return p ? b(p) : d;
  }, Ut = (a, d, p) => {
    const x = a && d._vnode ? void 0 : a && a.component ? a.component.uid : ic();
    al(() => {
      a == null ? d._vnode && Te(d._vnode, null, null, !0) : C(
        d._vnode || null,
        a,
        d,
        null,
        null,
        null,
        p
      ), d._vnode = a;
    }, x);
  }, ie = {
    p: C,
    um: Te,
    m: We,
    r: Vt,
    mt: wt,
    mc: A,
    pc: Y,
    pbc: se,
    n: st,
    o: e
  };
  return {
    render: Ut,
    hydrate: void 0,
    createApp: $l(Ut)
  };
}
function os({ type: e, props: t }, n) {
  return n === "svg" && e === "foreignObject" || n === "mathml" && e === "annotation-xml" && t && t.encoding && t.encoding.includes("html") ? void 0 : n;
}
function St({ effect: e, job: t }, n) {
  n ? (e.flags |= 32, t.flags |= 4) : (e.flags &= -33, t.flags &= -5);
}
function Ql(e, t) {
  return (!e || e && !e.pendingBranch) && t && !t.persisted;
}
function Vo(e, t, n = !1) {
  const s = e.children, r = t.children;
  if (F(s) && F(r))
    for (let o = 0; o < s.length; o++) {
      const i = s[o];
      let l = r[o];
      l.shapeFlag & 1 && !l.dynamicChildren && ((l.patchFlag <= 0 || l.patchFlag === 32) && (l = r[o] = at(r[o]), l.el = i.el), !n && l.patchFlag !== -2 && Vo(i, l)), l.type === Kn && (l.patchFlag === -1 && (l = r[o] = at(l)), l.el = i.el), l.type === Le && !l.el && (l.el = i.el);
    }
}
function Zl(e) {
  const t = e.slice(), n = [0];
  let s, r, o, i, l;
  const c = e.length;
  for (s = 0; s < c; s++) {
    const u = e[s];
    if (u !== 0) {
      if (r = n[n.length - 1], e[r] < u) {
        t[s] = r, n.push(s);
        continue;
      }
      for (o = 0, i = n.length - 1; o < i; )
        l = o + i >> 1, e[n[l]] < u ? o = l + 1 : i = l;
      u < e[n[o]] && (o > 0 && (t[s] = n[o - 1]), n[o] = s);
    }
  }
  for (o = n.length, i = n[o - 1]; o-- > 0; )
    n[o] = i, i = t[i];
  return n;
}
function Uo(e) {
  const t = e.subTree.component;
  if (t)
    return t.asyncDep && !t.asyncResolved ? t : Uo(t);
}
function pr(e) {
  if (e)
    for (let t = 0; t < e.length; t++)
      e[t].flags |= 8;
}
function on(e) {
  if (e.placeholder)
    return e.placeholder;
  const t = e.component;
  if (t)
    return on(t.subTree);
  const n = e.suspense;
  return n && n.activeBranch ? on(n.activeBranch) : null;
}
const Ko = (e) => e.__isSuspense, ec = /* @__PURE__ */ Symbol("_deferredEnter");
function tc(e, t) {
  t && t.pendingBranch ? F(e) ? t.effects.push(...e) : t.effects.push(e) : cl(e);
}
const fe = /* @__PURE__ */ Symbol.for("v-fgt"), Kn = /* @__PURE__ */ Symbol.for("v-txt"), Le = /* @__PURE__ */ Symbol.for("v-cmt"), is = /* @__PURE__ */ Symbol.for("v-stc"), dt = [];
let Oe = null;
function le(e = !1) {
  dt.push(Oe = e ? null : []);
}
function Us() {
  dt.pop(), Oe = dt[dt.length - 1] || null;
}
let fn = 1;
function Mn(e, t = !1) {
  fn += e, e < 0 && Oe && t && (Oe.hasOnce = !0);
}
function Go(e) {
  return e.dynamicChildren = fn > 0 ? Oe || Et : null, Us(), fn > 0 && Oe && Oe.push(e), e;
}
function be(e, t, n, s, r, o) {
  return Go(
    Ee(
      e,
      t,
      n,
      s,
      r,
      o,
      !0
    )
  );
}
function dn(e, t, n, s, r) {
  return Go(
    oe(
      e,
      t,
      n,
      s,
      r,
      !0
    )
  );
}
function Ht(e) {
  return e ? e.__v_isVNode === !0 : !1;
}
function Gt(e, t) {
  return e.type === t.type && e.key === t.key;
}
const qo = ({ key: e }) => e ?? null, Cn = ({
  ref: e,
  ref_key: t,
  ref_for: n
}) => (typeof e == "number" && (e = "" + e), e != null ? ce(e) || /* @__PURE__ */ _e(e) || H(e) ? { i: ve, r: e, k: t, f: !!n } : e : null);
function Ee(e, t = null, n = null, s = 0, r = null, o = e === fe ? 0 : 1, i = !1, l = !1) {
  const c = {
    __v_isVNode: !0,
    __v_skip: !0,
    type: e,
    props: t,
    key: t && qo(t),
    ref: t && Cn(t),
    scopeId: vo,
    slotScopeIds: null,
    children: n,
    component: null,
    suspense: null,
    ssContent: null,
    ssFallback: null,
    dirs: null,
    transition: null,
    el: null,
    anchor: null,
    target: null,
    targetStart: null,
    targetAnchor: null,
    staticCount: 0,
    shapeFlag: o,
    patchFlag: s,
    dynamicProps: r,
    dynamicChildren: null,
    appContext: null,
    ctx: ve
  };
  return l ? (Rn(c, n), o & 128 && e.normalize(c)) : n && (c.shapeFlag |= ce(n) ? 8 : 16), fn > 0 && // avoid a block node from tracking itself
  !i && // has current parent block
  Oe && // presence of a patch flag indicates this node needs patching on updates.
  // component nodes also should always be patched, because even if the
  // component doesn't need to update, it needs to persist the instance on to
  // the next vnode so that it can be properly unmounted later.
  (c.patchFlag > 0 || o & 6) && // the EVENTS flag is only for hydration and if it is the only flag, the
  // vnode should not be considered dynamic due to handler caching.
  c.patchFlag !== 32 && Oe.push(c), c;
}
const oe = nc;
function nc(e, t = null, n = null, s = 0, r = null, o = !1) {
  if ((!e || e === El) && (e = Le), Ht(e)) {
    const l = Wt(
      e,
      t,
      !0
      /* mergeRef: true */
    );
    return n && Rn(l, n), fn > 0 && !o && Oe && (l.shapeFlag & 6 ? Oe[Oe.indexOf(e)] = l : Oe.push(l)), l.patchFlag = -2, l;
  }
  if (hc(e) && (e = e.__vccOpts), t) {
    t = sc(t);
    let { class: l, style: c } = t;
    l && !ce(l) && (t.class = yt(l)), Q(c) && (/* @__PURE__ */ zs(c) && !F(c) && (c = he({}, c)), t.style = Is(c));
  }
  const i = ce(e) ? 1 : Ko(e) ? 128 : Vn(e) ? 64 : Q(e) ? 4 : H(e) ? 2 : 0;
  return Ee(
    e,
    t,
    n,
    s,
    r,
    i,
    o,
    !0
  );
}
function sc(e) {
  return e ? /* @__PURE__ */ zs(e) ? he({}, e) : Fo(e) ? he({}, e[Qo] || e) : e : null;
}
function Wt(e, t, n = !1, s = !1) {
  const { props: r, ref: o, patchFlag: i, children: l, transition: c } = e, u = t ? rc(r || {}, t) : r, f = {
    __v_isVNode: !0,
    __v_skip: !0,
    type: e.type,
    props: u,
    key: u && qo(u),
    ref: t && t.ref ? (
      // #2078 in the case of <component :is="vnode" ref="extra"/>
      // if the vnode itself already has a ref, cloneVNode will need to merge
      // the refs so the single vnode can be set on multiple refs
      n && o ? F(o) ? o.concat(Cn(t)) : [o, Cn(t)] : Cn(t)
    ) : o,
    scopeId: e.scopeId,
    slotScopeIds: e.slotScopeIds,
    children: l,
    target: e.target,
    targetStart: e.targetStart,
    targetAnchor: e.targetAnchor,
    staticCount: e.staticCount,
    shapeFlag: e.shapeFlag,
    // if the vnode is cloned with extra props, we can no longer assume its
    // existing patch flag to be reliable and need to add the FULL_PROPS flag.
    // note: preserve flag for fragments since they use the flag for children
    // fast paths only.
    patchFlag: t && e.type !== fe ? i === -1 ? 16 : i | 16 : i,
    dynamicProps: e.dynamicProps,
    dynamicChildren: e.dynamicChildren,
    appContext: e.appContext,
    dirs: e.dirs,
    transition: c,
    // These should technically only be non-null on mounted VNodes. However,
    // they *should* be copied for kept-alive vnodes. So we just always copy
    // them since them being non-null during a mount doesn't affect the logic as
    // they will simply be overwritten.
    component: e.component,
    suspense: e.suspense,
    ssContent: e.ssContent && Wt(e.ssContent),
    ssFallback: e.ssFallback && Wt(e.ssFallback),
    placeholder: e.placeholder,
    el: e.el,
    anchor: e.anchor,
    ctx: e.ctx,
    ce: e.ce,
    cacheIndex: e.cacheIndex
  };
  return c && s && nn(
    f,
    c.clone(f)
  ), f;
}
function gt(e = " ", t = 0) {
  return oe(Kn, null, e, t);
}
function jt(e = "", t = !1) {
  return t ? (le(), dn(Le, null, e)) : oe(Le, null, e);
}
function Je(e) {
  return e == null || typeof e == "boolean" ? oe(Le) : F(e) ? oe(
    fe,
    null,
    // #3666, avoid reference pollution when reusing vnode
    e.slice()
  ) : Ht(e) ? at(e) : oe(Kn, null, String(e));
}
function at(e) {
  return e.el === null && e.patchFlag !== -1 || e.memo ? e : Wt(e);
}
function Rn(e, t) {
  let n = 0;
  const { shapeFlag: s } = e;
  if (t == null)
    t = null;
  else if (F(t))
    n = 16;
  else if (typeof t == "object")
    if (s & 65) {
      const r = t.default;
      r && (r._c && (r._d = !1), Rn(e, r()), r._c && (r._d = !0));
      return;
    } else {
      n = 32;
      const r = t._;
      !r && !Fo(t) ? t._ctx = ve : r === 3 && ve && (ve.slots._ === 1 ? t._ = 1 : (t._ = 2, e.patchFlag |= 1024));
    }
  else if (H(t)) {
    if (s & 65) {
      Rn(e, { default: t });
      return;
    }
    t = { default: t, _ctx: ve }, n = 32;
  } else
    t = String(t), s & 64 ? (n = 16, t = [gt(t)]) : n = 8;
  e.children = t, e.shapeFlag |= n;
}
function rc(...e) {
  const t = {};
  for (let n = 0; n < e.length; n++) {
    const s = e[n];
    for (const r in s)
      if (r === "class")
        t.class !== s.class && (t.class = yt([t.class, s.class]));
      else if (r === "style")
        t.style = Is([t.style, s.style]);
      else if (Nn(r)) {
        const o = t[r], i = s[r];
        i && o !== i && !(F(o) && o.includes(i)) ? t[r] = o ? [].concat(o, i) : i : i == null && o == null && // mergeProps({ 'onUpdate:modelValue': undefined }) should not retain
        // the model listener.
        !Fn(r) && (t[r] = i);
      } else r !== "" && (t[r] = s[r]);
  }
  return t;
}
function Ue(e, t, n, s = null) {
  He(e, t, 7, [
    n,
    s
  ]);
}
const oc = /* @__PURE__ */ Mo();
let Jo = 0;
function ic() {
  return Jo;
}
function lc(e, t, n) {
  const s = e.type, r = (t ? t.appContext : e.appContext) || oc, o = {
    uid: Jo++,
    vnode: e,
    type: s,
    parent: t,
    appContext: r,
    root: null,
    // to be immediately set
    next: null,
    subTree: null,
    // will be set synchronously right after creation
    effect: null,
    update: null,
    // will be set synchronously right after creation
    job: null,
    scope: new Mi(
      !0
      /* detached */
    ),
    render: null,
    proxy: null,
    exposed: null,
    exposeProxy: null,
    withProxy: null,
    provides: t ? t.provides : Object.create(r.provides),
    ids: t ? t.ids : ["", 0, 0],
    accessCache: null,
    renderCache: [],
    // local resolved assets
    components: null,
    directives: null,
    // resolved props and emits options
    propsOptions: Do(s, r),
    emitsOptions: Ro(s, r),
    // emit
    emit: null,
    // to be set immediately
    emitted: null,
    // props default value
    propsDefaults: ne,
    // inheritAttrs
    inheritAttrs: s.inheritAttrs,
    // state
    ctx: ne,
    data: ne,
    props: ne,
    attrs: ne,
    slots: ne,
    refs: ne,
    setupState: ne,
    setupContext: null,
    // suspense related
    suspense: n,
    suspenseId: n ? n.pendingId : 0,
    asyncDep: null,
    asyncResolved: !1,
    // lifecycle hooks
    // not using enums here because it results in computed properties
    isMounted: !1,
    isUnmounted: !1,
    isDeactivated: !1,
    bc: null,
    c: null,
    bm: null,
    m: null,
    bu: null,
    u: null,
    um: null,
    bum: null,
    da: null,
    a: null,
    rtg: null,
    rtc: null,
    ec: null,
    sp: null
  };
  return o.ctx = { _: o }, o.root = t ? t.root : o, o.emit = Fl.bind(null, o), e.ce && e.ce(o), o;
}
let Ce = null;
const cc = () => Ce || ve;
let Ln, hn;
{
  const e = Dn(), t = (n, s) => {
    let r;
    return (r = e[n]) || (r = e[n] = []), r.push(s), (o) => {
      r.length > 1 ? r.forEach((i) => i(o)) : r[0](o);
    };
  };
  Ln = t(
    "__VUE_INSTANCE_SETTERS__",
    (n) => Ce = n
  ), hn = t(
    "__VUE_SSR_SETTERS__",
    (n) => pn = n
  );
}
const bn = (e) => {
  const t = Ce;
  return Ln(e), e.scope.on(), () => {
    e.scope.off(), Ln(t);
  };
}, gr = () => {
  Ce && Ce.scope.off(), Ln(null);
};
function Yo(e) {
  return e.vnode.shapeFlag & 4;
}
let pn = !1;
function ac(e, t = !1, n = !1) {
  t && hn(t);
  const { props: s, children: r } = e.vnode, o = Yo(e);
  Vl(e, s, o, t), ql(e, r, n || t);
  const i = o ? uc(e, t) : void 0;
  return t && hn(!1), i;
}
function uc(e, t) {
  const n = e.type;
  e.accessCache = /* @__PURE__ */ Object.create(null), e.proxy = new Proxy(e.ctx, Pl);
  const { setup: s } = n;
  if (s) {
    Qe();
    const r = e.setupContext = s.length > 1 ? dc(e) : null, o = bn(e), i = mn(
      s,
      e,
      0,
      [
        e.props,
        r
      ]
    ), l = Vr(i);
    if (Ze(), o(), (l || e.sp) && !zt(e) && So(e), l) {
      if (i.then(gr, gr), t)
        return i.then((c) => {
          hn(!0);
          try {
            mr(e, c, t);
          } finally {
            hn(!1);
          }
        }).catch((c) => {
          Bn(c, e, 0, void 0, !0), e.ssrRender = (u, f) => f("<!---->");
        });
      e.asyncDep = i;
    } else
      mr(e, i);
  } else
    Xo(e);
}
function mr(e, t, n) {
  H(t) ? e.type.__ssrInlineRender ? e.ssrRender = t : e.render = t : Q(t) && (e.setupState = po(t)), Xo(e);
}
function Xo(e, t, n) {
  const s = e.type;
  e.render || (e.render = s.render || Xe);
  {
    const r = bn(e);
    Qe();
    try {
      Ol(e);
    } finally {
      Ze(), r();
    }
  }
}
const Qo = /* @__PURE__ */ Symbol(), fc = {
  get(e, t) {
    return xe(e, "get", ""), t === Qo ? e : e[t];
  }
};
function dc(e) {
  const t = (n) => {
    e.exposed = n || {};
  };
  return {
    attrs: new Proxy(e.attrs, fc),
    slots: e.slots,
    emit: e.emit,
    expose: t
  };
}
function Gn(e) {
  return e.exposed ? e.exposeProxy || (e.exposeProxy = new Proxy(po(Xi(e.exposed)), {
    get(t, n) {
      if (n in t)
        return t[n];
      if (n in rn)
        return rn[n](e);
    },
    has(t, n) {
      return n in t || n in rn;
    }
  })) : e.proxy;
}
function hc(e) {
  return H(e) && "__vccOpts" in e;
}
const Ft = (e, t) => /* @__PURE__ */ nl(e, t, pn);
function ks(e, t, n) {
  try {
    Mn(-1);
    const s = arguments.length;
    return s === 2 ? Q(t) && !F(t) ? Ht(t) ? oe(e, null, [t]) : oe(e, t) : oe(e, null, t) : (s > 3 ? n = Array.prototype.slice.call(arguments, 2) : s === 3 && Ht(n) && (n = [n]), oe(e, t, n));
  } finally {
    Mn(1);
  }
}
const pc = "3.5.45";
let Ss;
const br = typeof window < "u" && window.trustedTypes;
if (br)
  try {
    Ss = /* @__PURE__ */ br.createPolicy("vue", {
      createHTML: (e) => e
    });
  } catch {
  }
const Zo = Ss ? (e) => Ss.createHTML(e) : (e) => e, gc = "http://www.w3.org/2000/svg", mc = "http://www.w3.org/1998/Math/MathML", ct = typeof document < "u" ? document : null, yr = ct && /* @__PURE__ */ ct.createElement("template"), bc = {
  insert: (e, t, n) => {
    t.insertBefore(e, n || null);
  },
  remove: (e) => {
    const t = e.parentNode;
    t && t.removeChild(e);
  },
  createElement: (e, t, n, s) => {
    const r = t === "svg" ? ct.createElementNS(gc, e) : t === "mathml" ? ct.createElementNS(mc, e) : n ? ct.createElement(e, { is: n }) : ct.createElement(e);
    return e === "select" && s && s.multiple != null && r.setAttribute("multiple", s.multiple), r;
  },
  createText: (e) => ct.createTextNode(e),
  createComment: (e) => ct.createComment(e),
  setText: (e, t) => {
    e.nodeValue = t;
  },
  setElementText: (e, t) => {
    e.textContent = t;
  },
  parentNode: (e) => e.parentNode,
  nextSibling: (e) => e.nextSibling,
  querySelector: (e) => ct.querySelector(e),
  setScopeId(e, t) {
    e.setAttribute(t, "");
  },
  // __UNSAFE__
  // Reason: innerHTML.
  // Static content here can only come from compiled templates.
  // As long as the user only uses trusted templates, this is safe.
  insertStaticContent(e, t, n, s, r, o) {
    const i = n ? n.previousSibling : t.lastChild;
    if (r && (r === o || r.nextSibling))
      for (; t.insertBefore(r.cloneNode(!0), n), !(r === o || !(r = r.nextSibling)); )
        ;
    else {
      yr.innerHTML = Zo(
        s === "svg" ? `<svg>${e}</svg>` : s === "mathml" ? `<math>${e}</math>` : e
      );
      const l = yr.content;
      if (s === "svg" || s === "mathml") {
        const c = l.firstChild;
        for (; c.firstChild; )
          l.appendChild(c.firstChild);
        l.removeChild(c);
      }
      t.insertBefore(l, n);
    }
    return [
      // first
      i ? i.nextSibling : t.firstChild,
      // last
      n ? n.previousSibling : t.lastChild
    ];
  }
}, yc = /* @__PURE__ */ Symbol("_vtc");
function xc(e, t, n) {
  const s = e[yc];
  s && (t = (t ? [t, ...s] : [...s]).join(" ")), t == null ? e.removeAttribute("class") : n ? e.setAttribute("class", t) : e.className = t;
}
const jn = /* @__PURE__ */ Symbol("_vod"), ei = /* @__PURE__ */ Symbol("_vsh"), vc = {
  // used for prop mismatch check during hydration
  name: "show",
  beforeMount(e, { value: t }, { transition: n }) {
    e[jn] = e.style.display === "none" ? "" : e.style.display, n && t ? n.beforeEnter(e) : qt(e, t);
  },
  mounted(e, { value: t }, { transition: n }) {
    n && t && n.enter(e);
  },
  updated(e, { value: t, oldValue: n }, { transition: s }) {
    !t != !n && (s ? t ? (s.beforeEnter(e), qt(e, !0), s.enter(e)) : s.leave(e, () => {
      qt(e, !1);
    }) : qt(e, t));
  },
  beforeUnmount(e, { value: t }) {
    qt(e, t);
  }
};
function qt(e, t) {
  e.style.display = t ? e[jn] : "none", e[ei] = !t;
}
const _c = /* @__PURE__ */ Symbol(""), wc = /(?:^|;)\s*display\s*:/;
function kc(e, t, n) {
  const s = e.style, r = ce(n);
  let o = !1;
  if (n && !r) {
    if (t)
      if (ce(t))
        for (const i of t.split(";")) {
          const l = i.slice(0, i.indexOf(":")).trim();
          n[l] == null && Xt(s, l, "");
        }
      else
        for (const i in t)
          n[i] == null && Xt(s, i, "");
    for (const i in n) {
      i === "display" && (o = !0);
      const l = n[i];
      l != null ? Cc(
        e,
        i,
        !ce(t) && t ? t[i] : void 0,
        l
      ) || Xt(s, i, l) : Xt(s, i, "");
    }
  } else if (r) {
    if (t !== n) {
      const i = s[_c];
      i && (n += ";" + i), s.cssText = n, o = wc.test(n);
    }
  } else t && e.removeAttribute("style");
  jn in e && ((!r || t !== n) && (e[jn] = o ? s.display : ""), e[ei] && (s.display = "none"));
}
const _n = /\s*!important$/;
function Xt(e, t, n) {
  if (F(n))
    n.forEach((s) => Xt(e, t, s));
  else if (n == null && (n = ""), t.startsWith("--"))
    _n.test(n) ? e.setProperty(t, n.replace(_n, ""), "important") : e.setProperty(t, n);
  else {
    const s = Sc(e, t);
    _n.test(n) ? e.setProperty(
      Ot(s),
      n.replace(_n, ""),
      "important"
    ) : e[s] = n;
  }
}
const xr = ["Webkit", "Moz", "ms"], ls = {};
function Sc(e, t) {
  const n = ls[t];
  if (n)
    return n;
  let s = Fe(t);
  if (s !== "filter" && s in e)
    return ls[t] = s;
  s = Gr(s);
  for (let r = 0; r < xr.length; r++) {
    const o = xr[r] + s;
    if (o in e)
      return ls[t] = o;
  }
  return t;
}
function Cc(e, t, n, s) {
  return e.tagName === "TEXTAREA" && (t === "width" || t === "height") && ce(s) && n === s;
}
const vr = "http://www.w3.org/1999/xlink";
function _r(e, t, n, s, r, o = Pi(t)) {
  s && t.startsWith("xlink:") ? n == null ? e.removeAttributeNS(vr, t.slice(6, t.length)) : e.setAttributeNS(vr, t, n) : n == null || o && !Jr(n) ? e.removeAttribute(t) : e.setAttribute(
    t,
    o ? "" : De(n) ? String(n) : n
  );
}
function wr(e, t, n, s, r) {
  if (t === "innerHTML" || t === "textContent") {
    n != null && (e[t] = t === "innerHTML" ? Zo(n) : n);
    return;
  }
  const o = e.tagName;
  if (t === "value" && o !== "PROGRESS" && // custom elements may use _value internally
  !o.includes("-")) {
    const l = o === "OPTION" ? e.getAttribute("value") || "" : e.value, c = n == null ? (
      // #11647: value should be set as empty string for null and undefined,
      // but <input type="checkbox"> should be set as 'on'.
      e.type === "checkbox" ? "on" : ""
    ) : String(n);
    (l !== c || !("_value" in e)) && (e.value = c), n == null && e.removeAttribute(t), e._value = n;
    return;
  }
  let i = !1;
  if (n === "" || n == null) {
    const l = typeof e[t];
    l === "boolean" ? n = Jr(n) : n == null && l === "string" ? (n = "", i = !0) : l === "number" && (n = 0, i = !0);
  }
  try {
    e[t] = n;
  } catch {
  }
  i && e.removeAttribute(r || t);
}
function Ac(e, t, n, s) {
  e.addEventListener(t, n, s);
}
function Tc(e, t, n, s) {
  e.removeEventListener(t, n, s);
}
const kr = /* @__PURE__ */ Symbol("_vei");
function Ec(e, t, n, s, r = null) {
  const o = e[kr] || (e[kr] = {}), i = o[t];
  if (s && i)
    i.value = s;
  else {
    const [l, c] = Ic(t);
    if (s) {
      const u = o[t] = Lc(
        s,
        r
      );
      Ac(e, l, u, c);
    } else i && (Tc(e, l, i, c), o[t] = void 0);
  }
}
const Pc = /(Once|Passive|Capture)$/, Oc = /^on:?(?:Once|Passive|Capture)$/;
function Ic(e) {
  let t, n;
  for (; (n = e.match(Pc)) && !Oc.test(e); )
    t || (t = {}), e = e.slice(0, e.length - n[1].length), t[n[1].toLowerCase()] = !0;
  return [e[2] === ":" ? e.slice(3) : Ot(e.slice(2)), t];
}
let cs = 0;
const Mc = /* @__PURE__ */ Promise.resolve(), Rc = () => cs || (Mc.then(() => cs = 0), cs = Date.now());
function Lc(e, t) {
  const n = (s) => {
    if (!s._vts)
      s._vts = Date.now();
    else if (s._vts <= n.attached)
      return;
    const r = n.value;
    if (F(r)) {
      const o = s.stopImmediatePropagation;
      s.stopImmediatePropagation = () => {
        o.call(s), s._stopped = !0;
      };
      const i = r.slice(), l = [s];
      for (let c = 0; c < i.length && !s._stopped; c++) {
        const u = i[c];
        u && He(
          u,
          t,
          5,
          l
        );
      }
    } else
      He(
        r,
        t,
        5,
        [s]
      );
  };
  return n.value = e, n.attached = Rc(), n;
}
const Sr = (e) => e.charCodeAt(0) === 111 && e.charCodeAt(1) === 110 && // lowercase letter
e.charCodeAt(2) > 96 && e.charCodeAt(2) < 123, jc = (e, t, n, s, r, o) => {
  const i = r === "svg";
  t === "class" ? xc(e, s, i) : t === "style" ? kc(e, n, s) : Nn(t) ? Fn(t) || Ec(e, t, n, s, o) : (t[0] === "." ? (t = t.slice(1), !0) : t[0] === "^" ? (t = t.slice(1), !1) : $c(e, t, s, i)) ? (wr(e, t, s), !e.tagName.includes("-") && (t === "value" || t === "checked" || t === "selected") && _r(e, t, s, i, o, t !== "value")) : /* #11081 force set props for possible async custom element */ e._isVueCE && // #12408 check if it's declared prop or it's async custom element
  (Nc(e, t) || // @ts-expect-error _def is private
  e._def.__asyncLoader && (/[A-Z]/.test(t) || !ce(s))) ? wr(e, Fe(t), s, o, t) : (t === "true-value" ? e._trueValue = s : t === "false-value" && (e._falseValue = s), _r(e, t, s, i));
};
function $c(e, t, n, s) {
  if (s)
    return !!(t === "innerHTML" || t === "textContent" || t in e && Sr(t) && H(n));
  if (t === "spellcheck" || t === "draggable" || t === "translate" || t === "autocorrect" || t === "sandbox" && e.tagName === "IFRAME" || t === "form" || t === "list" && e.tagName === "INPUT" || t === "type" && e.tagName === "TEXTAREA")
    return !1;
  if (t === "width" || t === "height") {
    const r = e.tagName;
    if (r === "IMG" || r === "VIDEO" || r === "CANVAS" || r === "SOURCE")
      return !1;
  }
  return Sr(t) && ce(n) ? !1 : t in e;
}
function Nc(e, t) {
  const n = (
    // @ts-expect-error _def is private
    e._def.props
  );
  if (!n)
    return !1;
  const s = Fe(t);
  return Array.isArray(n) ? n.some((r) => Fe(r) === s) : Object.keys(n).some((r) => Fe(r) === s);
}
const Fc = /* @__PURE__ */ he({ patchProp: jc }, bc);
let Cr;
function zc() {
  return Cr || (Cr = Yl(Fc));
}
const Dc = ((...e) => {
  const t = zc().createApp(...e), { mount: n } = t;
  return t.mount = (s) => {
    const r = Wc(s);
    if (!r) return;
    const o = t._component;
    !H(o) && !o.render && !o.template && (o.template = r.innerHTML), r.nodeType === 1 && (r.textContent = "");
    const i = n(r, !1, Hc(r));
    return r instanceof Element && (r.removeAttribute("v-cloak"), r.setAttribute("data-v-app", "")), i;
  }, t;
});
function Hc(e) {
  if (e instanceof SVGElement)
    return "svg";
  if (typeof MathMLElement == "function" && e instanceof MathMLElement)
    return "mathml";
}
function Wc(e) {
  return ce(e) ? document.querySelector(e) : e;
}
const Ct = {
  xmlns: "http://www.w3.org/2000/svg",
  width: 24,
  height: 24,
  viewBox: "0 0 24 24",
  fill: "none",
  stroke: "currentColor",
  "stroke-width": 2,
  "stroke-linecap": "round",
  "stroke-linejoin": "round"
};
const Ar = (...e) => e.filter((t, n, s) => !!t && t.trim() !== "" && s.indexOf(t) === n).join(" ").trim();
function as(e) {
  return e != null;
}
function Bc(e, t = {}) {
  const n = t.attributeNames ?? {}, s = (b) => n[b] ?? b, r = e.size ?? e.width ?? Ct.width, o = e.size ?? e.height ?? Ct.height, i = e.aliases?.filter((b) => typeof b == "string" && b.trim() !== "").map((b) => `lucide-${b}`) ?? [], l = [...e.name ? [`lucide-${e.name}`] : [], ...i], c = t.className?.split(" ").filter(Boolean) ?? [], u = t.includeDefaultClasses === !1 ? Ar(...c) : Ar("lucide", ...l, ...c), f = t.absoluteStrokeWidth ? Number(t.strokeWidth ?? Ct["stroke-width"]) * Number(e.size ?? e.width ?? Ct.width) / Number(t.size ?? t.width ?? Ct.width) : t.strokeWidth ?? Ct["stroke-width"];
  return [
    "svg",
    {
      ...Object.entries(Ct).reduce((b, [w, L]) => (b[s(w)] = L, b), {}),
      ..."color" in t && t.color && {
        [s("stroke")]: t.color
      },
      ..."size" in t && as(t.size) && {
        [s("width")]: t.size,
        [s("height")]: t.size
      },
      ..."width" in t && as(t.width) && {
        [s("width")]: t.width
      },
      ..."height" in t && as(t.height) && {
        [s("height")]: t.height
      },
      [s("stroke-width")]: f,
      ...u && {
        [s("class")]: u
      },
      [s("viewBox")]: `0 0 ${r} ${o}`,
      ...t.hasA11yProp === !1 ? {
        [s("aria-hidden")]: "true"
      } : {},
      ..."attributes" in t && t.attributes
    },
    e.node.map((b) => {
      const [w, L, C] = b, $ = t.nonScalingStroke ? { [s("vector-effect")]: "non-scaling-stroke", ...L } : L;
      return C ? [w, $, C] : [w, $];
    })
  ];
}
const Vc = (e) => {
  for (const t in e)
    if (t.startsWith("aria-") || t === "role" || t === "title")
      return !0;
  return !1;
};
const wn = (e) => e === "";
const Uc = (e) => e?.replace(/([a-z0-9])([A-Z])/g, "$1-$2").toLowerCase();
const Kc = /* @__PURE__ */ Symbol("lucide-icons");
function Gc() {
  return tn(Kc, {});
}
const qc = ({
  name: e,
  iconNode: t,
  "icon-node": n,
  icon: s = {
    name: e && Uc(e),
    node: t ?? n ?? [],
    size: 24,
    aliases: []
  },
  absoluteStrokeWidth: r,
  "absolute-stroke-width": o,
  nonScalingStroke: i,
  "non-scaling-stroke": l,
  strokeWidth: c,
  "stroke-width": u,
  size: f,
  width: h = f,
  height: b = f,
  color: w,
  ...L
}, { slots: C }) => {
  const {
    size: $,
    color: D,
    strokeWidth: j = 2,
    absoluteStrokeWidth: W = !1,
    nonScalingStroke: O = !1,
    class: B = ""
  } = Gc(), N = wn(r) || wn(o) || r === !0 || o === !0 || W === !0, G = wn(i) || wn(l) || i === !0 || l === !0 || O === !0;
  delete L.class;
  const A = C.default?.(), [, v, se = []] = Bc(s, {
    color: w ?? D,
    width: h ?? f ?? $,
    height: b ?? f ?? $,
    strokeWidth: c ?? u ?? j,
    absoluteStrokeWidth: N,
    nonScalingStroke: G,
    className: B,
    hasA11yProp: A != null && A.length > 0 || Vc(L),
    attributes: L
  });
  return ks("svg", v, [
    ...se.map(([U, Ae]) => ks(U, Ae)),
    ...A ?? []
  ]);
};
function Bt(e, t = []) {
  const n = typeof e == "string" ? {
    name: e,
    node: t
  } : e;
  return (s, { slots: r }) => ks(
    qc,
    {
      ...s,
      icon: n
    },
    r.default ? { default: r.default } : void 0
  );
}
const Jc = {
  name: "check",
  size: 24,
  node: [["path", { d: "M20 6 9 17l-5-5", key: "1gmf2c" }]]
}, Yc = Bt(Jc);
const Xc = {
  name: "circle-exclamation-point",
  size: 24,
  node: [
    ["circle", { cx: "12", cy: "12", r: "10", key: "1mglay" }],
    ["line", { x1: "12", x2: "12", y1: "8", y2: "12", key: "1pkeuh" }],
    ["line", { x1: "12", x2: "12.01", y1: "16", y2: "16", key: "4dfq90" }]
  ],
  aliases: ["alert-circle", "circle-alert"]
}, Tr = Bt(Xc);
const Qc = {
  name: "refresh-cw",
  size: 24,
  node: [
    ["path", { d: "M3 12a9 9 0 0 1 9-9 9.75 9.75 0 0 1 6.74 2.74L21 8", key: "v9h5vc" }],
    ["path", { d: "M21 3v5h-5", key: "1q7to0" }],
    ["path", { d: "M21 12a9 9 0 0 1-9 9 9.75 9.75 0 0 1-6.74-2.74L3 16", key: "3uifl3" }],
    ["path", { d: "M8 16H3v5", key: "1cv678" }]
  ]
}, Zc = Bt(Qc);
const ea = {
  name: "triangle-exclamation-point",
  size: 24,
  node: [
    [
      "path",
      {
        d: "m21.73 18-8-14a2 2 0 0 0-3.48 0l-8 14A2 2 0 0 0 4 21h16a2 2 0 0 0 1.73-3",
        key: "wmoenq"
      }
    ],
    ["path", { d: "M12 9v4", key: "juzpu7" }],
    ["path", { d: "M12 17h.01", key: "p32p05" }]
  ],
  aliases: ["alert-triangle", "triangle-alert"]
}, Er = Bt(ea);
const ta = {
  name: "wrench",
  size: 24,
  node: [
    [
      "path",
      {
        d: "M14.7 6.3a1 1 0 0 0 0 1.4l1.6 1.6a1 1 0 0 0 1.4 0l3.106-3.105c.32-.322.863-.22.983.218a6 6 0 0 1-8.259 7.057l-7.91 7.91a1 1 0 0 1-2.999-3l7.91-7.91a6 6 0 0 1 7.057-8.259c.438.12.54.662.219.984z",
        key: "1ngwbx"
      }
    ]
  ]
}, na = Bt(ta);
const sa = {
  name: "x",
  size: 24,
  node: [
    ["path", { d: "M18 6 6 18", key: "1bl5f8" }],
    ["path", { d: "m6 6 12 12", key: "d8bk6v" }]
  ]
}, ra = Bt(sa);
function ti(e) {
  var t, n, s = "";
  if (typeof e == "string" || typeof e == "number") s += e;
  else if (typeof e == "object") if (Array.isArray(e)) {
    var r = e.length;
    for (t = 0; t < r; t++) e[t] && (n = ti(e[t])) && (s && (s += " "), s += n);
  } else for (n in e) e[n] && (s && (s += " "), s += n);
  return s;
}
function ni() {
  for (var e, t, n = 0, s = "", r = arguments.length; n < r; n++) (e = arguments[n]) && (t = ti(e)) && (s && (s += " "), s += t);
  return s;
}
const oa = (e, t) => {
  const n = new Array(e.length + t.length);
  for (let s = 0; s < e.length; s++)
    n[s] = e[s];
  for (let s = 0; s < t.length; s++)
    n[e.length + s] = t[s];
  return n;
}, ia = (e, t) => ({
  classGroupId: e,
  validator: t
}), si = (e = /* @__PURE__ */ new Map(), t = null, n) => ({
  nextPart: e,
  validators: t,
  classGroupId: n
}), $n = "-", Pr = [], la = "arbitrary..", ca = (e) => {
  const t = ua(e), {
    conflictingClassGroups: n,
    conflictingClassGroupModifiers: s
  } = e;
  return {
    getClassGroupId: (i) => {
      if (i.startsWith("[") && i.endsWith("]"))
        return aa(i);
      const l = i.split($n), c = l[0] === "" && l.length > 1 ? 1 : 0;
      return ri(l, c, t);
    },
    getConflictingClassGroupIds: (i, l) => {
      if (l) {
        const c = s[i], u = n[i];
        return c ? u ? oa(u, c) : c : u || Pr;
      }
      return n[i] || Pr;
    }
  };
}, ri = (e, t, n) => {
  if (e.length - t === 0)
    return n.classGroupId;
  const r = e[t], o = n.nextPart.get(r);
  if (o) {
    const u = ri(e, t + 1, o);
    if (u) return u;
  }
  const i = n.validators;
  if (i === null)
    return;
  const l = t === 0 ? e.join($n) : e.slice(t).join($n), c = i.length;
  for (let u = 0; u < c; u++) {
    const f = i[u];
    if (f.validator(l))
      return f.classGroupId;
  }
}, aa = (e) => e.slice(1, -1).indexOf(":") === -1 ? void 0 : (() => {
  const t = e.slice(1, -1), n = t.indexOf(":"), s = t.slice(0, n);
  return s ? la + s : void 0;
})(), ua = (e) => {
  const {
    theme: t,
    classGroups: n
  } = e;
  return fa(n, t);
}, fa = (e, t) => {
  const n = si();
  for (const s in e) {
    const r = e[s];
    Ks(r, n, s, t);
  }
  return n;
}, Ks = (e, t, n, s) => {
  const r = e.length;
  for (let o = 0; o < r; o++) {
    const i = e[o];
    da(i, t, n, s);
  }
}, da = (e, t, n, s) => {
  if (typeof e == "string") {
    ha(e, t, n);
    return;
  }
  if (typeof e == "function") {
    pa(e, t, n, s);
    return;
  }
  ga(e, t, n, s);
}, ha = (e, t, n) => {
  const s = e === "" ? t : oi(t, e);
  s.classGroupId = n;
}, pa = (e, t, n, s) => {
  if (ma(e)) {
    Ks(e(s), t, n, s);
    return;
  }
  t.validators === null && (t.validators = []), t.validators.push(ia(n, e));
}, ga = (e, t, n, s) => {
  const r = Object.entries(e), o = r.length;
  for (let i = 0; i < o; i++) {
    const [l, c] = r[i];
    Ks(c, oi(t, l), n, s);
  }
}, oi = (e, t) => {
  let n = e;
  const s = t.split($n), r = s.length;
  for (let o = 0; o < r; o++) {
    const i = s[o];
    let l = n.nextPart.get(i);
    l || (l = si(), n.nextPart.set(i, l)), n = l;
  }
  return n;
}, ma = (e) => "isThemeGetter" in e && e.isThemeGetter === !0, ba = (e) => {
  if (e < 1)
    return {
      get: () => {
      },
      set: () => {
      }
    };
  let t = 0, n = /* @__PURE__ */ Object.create(null), s = /* @__PURE__ */ Object.create(null);
  const r = (o, i) => {
    n[o] = i, t++, t > e && (t = 0, s = n, n = /* @__PURE__ */ Object.create(null));
  };
  return {
    get(o) {
      let i = n[o];
      if (i !== void 0)
        return i;
      if ((i = s[o]) !== void 0)
        return r(o, i), i;
    },
    set(o, i) {
      o in n ? n[o] = i : r(o, i);
    }
  };
}, Cs = "!", Or = ":", ya = [], Ir = (e, t, n, s, r) => ({
  modifiers: e,
  hasImportantModifier: t,
  baseClassName: n,
  maybePostfixModifierPosition: s,
  isExternal: r
}), xa = (e) => {
  const {
    prefix: t,
    experimentalParseClassName: n
  } = e;
  let s = (r) => {
    const o = [];
    let i = 0, l = 0, c = 0, u;
    const f = r.length;
    for (let C = 0; C < f; C++) {
      const $ = r[C];
      if (i === 0 && l === 0) {
        if ($ === Or) {
          o.push(r.slice(c, C)), c = C + 1;
          continue;
        }
        if ($ === "/") {
          u = C;
          continue;
        }
      }
      $ === "[" ? i++ : $ === "]" ? i-- : $ === "(" ? l++ : $ === ")" && l--;
    }
    const h = o.length === 0 ? r : r.slice(c);
    let b = h, w = !1;
    h.endsWith(Cs) ? (b = h.slice(0, -1), w = !0) : (
      /**
       * In Tailwind CSS v3 the important modifier was at the start of the base class name. This is still supported for legacy reasons.
       * @see https://github.com/dcastil/tailwind-merge/issues/513#issuecomment-2614029864
       */
      h.startsWith(Cs) && (b = h.slice(1), w = !0)
    );
    const L = u && u > c ? u - c : void 0;
    return Ir(o, w, b, L);
  };
  if (t) {
    const r = t + Or, o = s;
    s = (i) => i.startsWith(r) ? o(i.slice(r.length)) : Ir(ya, !1, i, void 0, !0);
  }
  if (n) {
    const r = s;
    s = (o) => n({
      className: o,
      parseClassName: r
    });
  }
  return s;
}, va = (e) => {
  const t = /* @__PURE__ */ new Map();
  return e.orderSensitiveModifiers.forEach((n, s) => {
    t.set(n, 1e6 + s);
  }), (n) => {
    const s = [];
    let r = [];
    for (let o = 0; o < n.length; o++) {
      const i = n[o], l = i[0] === "[", c = t.has(i);
      l || c ? (r.length > 0 && (r.sort(), s.push(...r), r = []), s.push(i)) : r.push(i);
    }
    return r.length > 0 && (r.sort(), s.push(...r)), s;
  };
}, _a = (e) => ({
  cache: ba(e.cacheSize),
  parseClassName: xa(e),
  sortModifiers: va(e),
  postfixLookupClassGroupIds: wa(e),
  ...ca(e)
}), wa = (e) => {
  const t = /* @__PURE__ */ Object.create(null), n = e.postfixLookupClassGroups;
  if (n)
    for (let s = 0; s < n.length; s++)
      t[n[s]] = !0;
  return t;
}, ka = /\s+/, Sa = (e, t) => {
  const {
    parseClassName: n,
    getClassGroupId: s,
    getConflictingClassGroupIds: r,
    sortModifiers: o,
    postfixLookupClassGroupIds: i
  } = t, l = [], c = e.trim().split(ka);
  let u = "";
  for (let f = c.length - 1; f >= 0; f -= 1) {
    const h = c[f], {
      isExternal: b,
      modifiers: w,
      hasImportantModifier: L,
      baseClassName: C,
      maybePostfixModifierPosition: $
    } = n(h);
    if (b) {
      u = h + (u.length > 0 ? " " + u : u);
      continue;
    }
    let D = !!$, j;
    if (D) {
      const G = C.substring(0, $);
      j = s(G);
      const A = j && i[j] ? s(C) : void 0;
      A && A !== j && (j = A, D = !1);
    } else
      j = s(C);
    if (!j) {
      if (!D) {
        u = h + (u.length > 0 ? " " + u : u);
        continue;
      }
      if (j = s(C), !j) {
        u = h + (u.length > 0 ? " " + u : u);
        continue;
      }
      D = !1;
    }
    const W = w.length === 0 ? "" : w.length === 1 ? w[0] : o(w).join(":"), O = L ? W + Cs : W, B = O + j;
    if (l.indexOf(B) > -1)
      continue;
    l.push(B);
    const N = r(j, D);
    for (let G = 0; G < N.length; ++G) {
      const A = N[G];
      l.push(O + A);
    }
    u = h + (u.length > 0 ? " " + u : u);
  }
  return u;
}, Ca = (...e) => {
  let t = 0, n, s, r = "";
  for (; t < e.length; )
    (n = e[t++]) && (s = ii(n)) && (r && (r += " "), r += s);
  return r;
}, ii = (e) => {
  if (typeof e == "string")
    return e;
  let t, n = "";
  for (let s = 0; s < e.length; s++)
    e[s] && (t = ii(e[s])) && (n && (n += " "), n += t);
  return n;
}, Aa = (e, ...t) => {
  let n, s, r, o;
  const i = (c) => {
    const u = t.reduce((f, h) => h(f), e());
    return n = _a(u), s = n.cache.get, r = n.cache.set, o = l, l(c);
  }, l = (c) => {
    const u = s(c);
    if (u)
      return u;
    const f = Sa(c, n);
    return r(c, f), f;
  };
  return o = i, (...c) => o(Ca(...c));
}, Ta = [], de = (e) => {
  const t = (n) => n[e] || Ta;
  return t.isThemeGetter = !0, t.themeKey = e, t;
}, li = /^\[(?:(\w[\w-]*):)?(.+)\]$/i, ci = /^\((?:(\w[\w-]*):)?(.+)\)$/i, Ea = /^\d+(?:\.\d+)?\/\d+(?:\.\d+)?$/, Pa = /^(\d+(\.\d+)?)?(xs|sm|md|lg|xl)$/, Oa = /\d+(%|px|r?em|[sdl]?v([hwib]|min|max)|pt|pc|in|cm|mm|cap|ch|ex|r?lh|cq(w|h|i|b|min|max))|\b(calc|min|max|clamp)\(.+\)|^0$/, Ia = /^(rgba?|hsla?|hwb|(ok)?(lab|lch)|color-mix|color|light-dark)\(.+\)$/, Ma = /^(inset_)?-?((\d+)?\.?(\d+)[a-z]+|0)_-?((\d+)?\.?(\d+)[a-z]+|0)/, Ra = /^(url|image|image-set|cross-fade|element|(repeating-)?(linear|radial|conic)-gradient)\(.+\)$/, pt = (e) => Ea.test(e), V = (e) => !!e && !Number.isNaN(Number(e)), Ke = (e) => !!e && Number.isInteger(Number(e)), us = (e) => e.endsWith("%") && V(e.slice(0, -1)), it = (e) => Pa.test(e), ai = () => !0, La = (e) => (
  // `colorFunctionRegex` check is necessary because color functions can have percentages in them which which would be incorrectly classified as lengths.
  // For example, `hsl(0 0% 0%)` would be classified as a length without this check.
  // I could also use lookbehind assertion in `lengthUnitRegex` but that isn't supported widely enough.
  Oa.test(e) && !Ia.test(e)
), Gs = () => !1, ja = (e) => Ma.test(e), $a = (e) => Ra.test(e), Na = (e) => !E(e) && !P(e), Fa = (e) => e.startsWith("@container") && (e[10] === "/" && e[11] !== void 0 || e[11] === "s" && e[16] !== void 0 && e.startsWith("-size/", 10) || e[11] === "n" && e[18] !== void 0 && e.startsWith("-normal/", 10)), za = (e) => _t(e, di, Gs), E = (e) => li.test(e), At = (e) => _t(e, hi, La), Mr = (e) => _t(e, Ga, V), Da = (e) => _t(e, gi, ai), Ha = (e) => _t(e, pi, Gs), Rr = (e) => _t(e, ui, Gs), Wa = (e) => _t(e, fi, $a), kn = (e) => _t(e, mi, ja), P = (e) => ci.test(e), Jt = (e) => It(e, hi), Ba = (e) => It(e, pi), Lr = (e) => It(e, ui), Va = (e) => It(e, di), Ua = (e) => It(e, fi), Sn = (e) => It(e, mi, !0), Ka = (e) => It(e, gi, !0), _t = (e, t, n) => {
  const s = li.exec(e);
  return s ? s[1] ? t(s[1]) : n(s[2]) : !1;
}, It = (e, t, n = !1) => {
  const s = ci.exec(e);
  return s ? s[1] ? t(s[1]) : n : !1;
}, ui = (e) => e === "position" || e === "percentage", fi = (e) => e === "image" || e === "url", di = (e) => e === "length" || e === "size" || e === "bg-size", hi = (e) => e === "length", Ga = (e) => e === "number", pi = (e) => e === "family-name", gi = (e) => e === "number" || e === "weight", mi = (e) => e === "shadow", qa = () => {
  const e = de("color"), t = de("font"), n = de("text"), s = de("font-weight"), r = de("tracking"), o = de("leading"), i = de("breakpoint"), l = de("container"), c = de("spacing"), u = de("radius"), f = de("shadow"), h = de("inset-shadow"), b = de("text-shadow"), w = de("drop-shadow"), L = de("blur"), C = de("perspective"), $ = de("aspect"), D = de("ease"), j = de("animate"), W = () => ["auto", "avoid", "all", "avoid-page", "page", "left", "right", "column"], O = () => [
    "center",
    "top",
    "bottom",
    "left",
    "right",
    "top-left",
    // Deprecated since Tailwind CSS v4.1.0, see https://github.com/tailwindlabs/tailwindcss/pull/17378
    "left-top",
    "top-right",
    // Deprecated since Tailwind CSS v4.1.0, see https://github.com/tailwindlabs/tailwindcss/pull/17378
    "right-top",
    "bottom-right",
    // Deprecated since Tailwind CSS v4.1.0, see https://github.com/tailwindlabs/tailwindcss/pull/17378
    "right-bottom",
    "bottom-left",
    // Deprecated since Tailwind CSS v4.1.0, see https://github.com/tailwindlabs/tailwindcss/pull/17378
    "left-bottom"
  ], B = () => [...O(), P, E], N = () => ["auto", "hidden", "clip", "visible", "scroll"], G = () => ["auto", "contain", "none"], A = () => [P, E, c], v = () => [pt, "full", "auto", ...A()], se = () => [Ke, "none", "subgrid", P, E], U = () => ["auto", {
    span: ["full", Ke, P, E]
  }, Ke, P, E], Ae = () => [Ke, "auto", P, E], tt = () => ["auto", "min", "max", "fr", P, E], wt = () => ["start", "end", "center", "between", "around", "evenly", "stretch", "baseline", "center-safe", "end-safe"], nt = () => ["start", "end", "center", "stretch", "center-safe", "end-safe"], ee = () => ["auto", ...A()], q = () => [pt, "auto", "full", "dvw", "dvh", "lvw", "lvh", "svw", "svh", "min", "max", "fit", ...A()], Y = () => [l, pt, "screen", "full", "dvw", "lvw", "svw", "min", "max", "fit", ...A()], je = () => [pt, "screen", "full", "lh", "dvh", "lvh", "svh", "min", "max", "fit", ...A()], M = () => [e, P, E], We = () => [...O(), Lr, Rr, {
    position: [P, E]
  }], Te = () => ["no-repeat", {
    repeat: ["", "x", "y", "space", "round"]
  }], Vt = () => ["auto", "cover", "contain", Va, za, {
    size: [P, E]
  }], Mt = () => [us, Jt, At], ge = () => [
    // Deprecated since Tailwind CSS v4.0.0
    "",
    "none",
    "full",
    u,
    P,
    E
  ], ae = () => ["", V, Jt, At], st = () => ["solid", "dashed", "dotted", "double"], Ut = () => ["normal", "multiply", "screen", "overlay", "darken", "lighten", "color-dodge", "color-burn", "hard-light", "soft-light", "difference", "exclusion", "hue", "saturation", "color", "luminosity"], ie = () => [V, us, Lr, Rr], qn = () => [
    // Deprecated since Tailwind CSS v4.0.0
    "",
    "none",
    L,
    P,
    E
  ], a = () => ["none", V, P, E], d = () => ["none", V, P, E], p = () => [V, P, E], x = () => [pt, "full", ...A()];
  return {
    cacheSize: 500,
    theme: {
      animate: ["spin", "ping", "pulse", "bounce"],
      aspect: ["video"],
      blur: [it],
      breakpoint: [it],
      color: [ai],
      container: [it],
      "drop-shadow": [it],
      ease: ["in", "out", "in-out"],
      font: [Na],
      "font-weight": ["thin", "extralight", "light", "normal", "medium", "semibold", "bold", "extrabold", "black"],
      "inset-shadow": [it],
      leading: ["none", "tight", "snug", "normal", "relaxed", "loose"],
      perspective: ["dramatic", "near", "normal", "midrange", "distant", "none"],
      radius: [it],
      shadow: [it],
      spacing: ["px", V],
      text: [it],
      "text-shadow": [it],
      tracking: ["tighter", "tight", "normal", "wide", "wider", "widest"]
    },
    classGroups: {
      // --------------
      // --- Layout ---
      // --------------
      /**
       * Aspect Ratio
       * @see https://tailwindcss.com/docs/aspect-ratio
       */
      aspect: [{
        aspect: ["auto", "square", pt, E, P, $]
      }],
      /**
       * Container
       * @see https://tailwindcss.com/docs/container
       * @deprecated since Tailwind CSS v4.0.0
       */
      container: ["container"],
      /**
       * Container Type
       * @see https://tailwindcss.com/docs/responsive-design#container-queries
       */
      "container-type": [{
        "@container": ["", "normal", "size", P, E]
      }],
      /**
       * Container Name
       * @see https://tailwindcss.com/docs/responsive-design#named-containers
       */
      "container-named": [Fa],
      /**
       * Columns
       * @see https://tailwindcss.com/docs/columns
       */
      columns: [{
        columns: [V, "auto", E, P, l]
      }],
      /**
       * Break After
       * @see https://tailwindcss.com/docs/break-after
       */
      "break-after": [{
        "break-after": W()
      }],
      /**
       * Break Before
       * @see https://tailwindcss.com/docs/break-before
       */
      "break-before": [{
        "break-before": W()
      }],
      /**
       * Break Inside
       * @see https://tailwindcss.com/docs/break-inside
       */
      "break-inside": [{
        "break-inside": ["auto", "avoid", "avoid-page", "avoid-column"]
      }],
      /**
       * Box Decoration Break
       * @see https://tailwindcss.com/docs/box-decoration-break
       */
      "box-decoration": [{
        "box-decoration": ["slice", "clone"]
      }],
      /**
       * Box Sizing
       * @see https://tailwindcss.com/docs/box-sizing
       */
      box: [{
        box: ["border", "content"]
      }],
      /**
       * Display
       * @see https://tailwindcss.com/docs/display
       */
      display: ["block", "inline-block", "inline", "flex", "inline-flex", "table", "inline-table", "table-caption", "table-cell", "table-column", "table-column-group", "table-footer-group", "table-header-group", "table-row-group", "table-row", "flow-root", "grid", "inline-grid", "contents", "list-item", "hidden"],
      /**
       * Screen Reader Only
       * @see https://tailwindcss.com/docs/display#screen-reader-only
       */
      sr: ["sr-only", "not-sr-only"],
      /**
       * Floats
       * @see https://tailwindcss.com/docs/float
       */
      float: [{
        float: ["right", "left", "none", "start", "end"]
      }],
      /**
       * Clear
       * @see https://tailwindcss.com/docs/clear
       */
      clear: [{
        clear: ["left", "right", "both", "none", "start", "end"]
      }],
      /**
       * Isolation
       * @see https://tailwindcss.com/docs/isolation
       */
      isolation: ["isolate", "isolation-auto"],
      /**
       * Object Fit
       * @see https://tailwindcss.com/docs/object-fit
       */
      "object-fit": [{
        object: ["contain", "cover", "fill", "none", "scale-down"]
      }],
      /**
       * Object Position
       * @see https://tailwindcss.com/docs/object-position
       */
      "object-position": [{
        object: B()
      }],
      /**
       * Overflow
       * @see https://tailwindcss.com/docs/overflow
       */
      overflow: [{
        overflow: N()
      }],
      /**
       * Overflow X
       * @see https://tailwindcss.com/docs/overflow
       */
      "overflow-x": [{
        "overflow-x": N()
      }],
      /**
       * Overflow Y
       * @see https://tailwindcss.com/docs/overflow
       */
      "overflow-y": [{
        "overflow-y": N()
      }],
      /**
       * Overscroll Behavior
       * @see https://tailwindcss.com/docs/overscroll-behavior
       */
      overscroll: [{
        overscroll: G()
      }],
      /**
       * Overscroll Behavior X
       * @see https://tailwindcss.com/docs/overscroll-behavior
       */
      "overscroll-x": [{
        "overscroll-x": G()
      }],
      /**
       * Overscroll Behavior Y
       * @see https://tailwindcss.com/docs/overscroll-behavior
       */
      "overscroll-y": [{
        "overscroll-y": G()
      }],
      /**
       * Position
       * @see https://tailwindcss.com/docs/position
       */
      position: ["static", "fixed", "absolute", "relative", "sticky"],
      /**
       * Inset
       * @see https://tailwindcss.com/docs/top-right-bottom-left
       */
      inset: [{
        inset: v()
      }],
      /**
       * Inset Inline
       * @see https://tailwindcss.com/docs/top-right-bottom-left
       */
      "inset-x": [{
        "inset-x": v()
      }],
      /**
       * Inset Block
       * @see https://tailwindcss.com/docs/top-right-bottom-left
       */
      "inset-y": [{
        "inset-y": v()
      }],
      /**
       * Inset Inline Start
       * @see https://tailwindcss.com/docs/top-right-bottom-left
       * @todo class group will be renamed to `inset-s` in next major release
       */
      start: [{
        "inset-s": v(),
        /**
         * @deprecated since Tailwind CSS v4.2.0 in favor of `inset-s-*` utilities.
         * @see https://github.com/tailwindlabs/tailwindcss/pull/19613
         */
        start: v()
      }],
      /**
       * Inset Inline End
       * @see https://tailwindcss.com/docs/top-right-bottom-left
       * @todo class group will be renamed to `inset-e` in next major release
       */
      end: [{
        "inset-e": v(),
        /**
         * @deprecated since Tailwind CSS v4.2.0 in favor of `inset-e-*` utilities.
         * @see https://github.com/tailwindlabs/tailwindcss/pull/19613
         */
        end: v()
      }],
      /**
       * Inset Block Start
       * @see https://tailwindcss.com/docs/top-right-bottom-left
       */
      "inset-bs": [{
        "inset-bs": v()
      }],
      /**
       * Inset Block End
       * @see https://tailwindcss.com/docs/top-right-bottom-left
       */
      "inset-be": [{
        "inset-be": v()
      }],
      /**
       * Top
       * @see https://tailwindcss.com/docs/top-right-bottom-left
       */
      top: [{
        top: v()
      }],
      /**
       * Right
       * @see https://tailwindcss.com/docs/top-right-bottom-left
       */
      right: [{
        right: v()
      }],
      /**
       * Bottom
       * @see https://tailwindcss.com/docs/top-right-bottom-left
       */
      bottom: [{
        bottom: v()
      }],
      /**
       * Left
       * @see https://tailwindcss.com/docs/top-right-bottom-left
       */
      left: [{
        left: v()
      }],
      /**
       * Visibility
       * @see https://tailwindcss.com/docs/visibility
       */
      visibility: ["visible", "invisible", "collapse"],
      /**
       * Z-Index
       * @see https://tailwindcss.com/docs/z-index
       */
      z: [{
        z: [Ke, "auto", P, E]
      }],
      // ------------------------
      // --- Flexbox and Grid ---
      // ------------------------
      /**
       * Flex Basis
       * @see https://tailwindcss.com/docs/flex-basis
       */
      basis: [{
        basis: [pt, "full", "auto", l, ...A()]
      }],
      /**
       * Flex Direction
       * @see https://tailwindcss.com/docs/flex-direction
       */
      "flex-direction": [{
        flex: ["row", "row-reverse", "col", "col-reverse"]
      }],
      /**
       * Flex Wrap
       * @see https://tailwindcss.com/docs/flex-wrap
       */
      "flex-wrap": [{
        flex: ["nowrap", "wrap", "wrap-reverse"]
      }],
      /**
       * Flex
       * @see https://tailwindcss.com/docs/flex
       */
      flex: [{
        flex: [V, pt, "auto", "initial", "none", E]
      }],
      /**
       * Flex Grow
       * @see https://tailwindcss.com/docs/flex-grow
       */
      grow: [{
        grow: ["", V, P, E]
      }],
      /**
       * Flex Shrink
       * @see https://tailwindcss.com/docs/flex-shrink
       */
      shrink: [{
        shrink: ["", V, P, E]
      }],
      /**
       * Order
       * @see https://tailwindcss.com/docs/order
       */
      order: [{
        order: [Ke, "first", "last", "none", P, E]
      }],
      /**
       * Grid Template Columns
       * @see https://tailwindcss.com/docs/grid-template-columns
       */
      "grid-cols": [{
        "grid-cols": se()
      }],
      /**
       * Grid Column Start / End
       * @see https://tailwindcss.com/docs/grid-column
       */
      "col-start-end": [{
        col: U()
      }],
      /**
       * Grid Column Start
       * @see https://tailwindcss.com/docs/grid-column
       */
      "col-start": [{
        "col-start": Ae()
      }],
      /**
       * Grid Column End
       * @see https://tailwindcss.com/docs/grid-column
       */
      "col-end": [{
        "col-end": Ae()
      }],
      /**
       * Grid Template Rows
       * @see https://tailwindcss.com/docs/grid-template-rows
       */
      "grid-rows": [{
        "grid-rows": se()
      }],
      /**
       * Grid Row Start / End
       * @see https://tailwindcss.com/docs/grid-row
       */
      "row-start-end": [{
        row: U()
      }],
      /**
       * Grid Row Start
       * @see https://tailwindcss.com/docs/grid-row
       */
      "row-start": [{
        "row-start": Ae()
      }],
      /**
       * Grid Row End
       * @see https://tailwindcss.com/docs/grid-row
       */
      "row-end": [{
        "row-end": Ae()
      }],
      /**
       * Grid Auto Flow
       * @see https://tailwindcss.com/docs/grid-auto-flow
       */
      "grid-flow": [{
        "grid-flow": ["row", "col", "dense", "row-dense", "col-dense"]
      }],
      /**
       * Grid Auto Columns
       * @see https://tailwindcss.com/docs/grid-auto-columns
       */
      "auto-cols": [{
        "auto-cols": tt()
      }],
      /**
       * Grid Auto Rows
       * @see https://tailwindcss.com/docs/grid-auto-rows
       */
      "auto-rows": [{
        "auto-rows": tt()
      }],
      /**
       * Gap
       * @see https://tailwindcss.com/docs/gap
       */
      gap: [{
        gap: A()
      }],
      /**
       * Gap X
       * @see https://tailwindcss.com/docs/gap
       */
      "gap-x": [{
        "gap-x": A()
      }],
      /**
       * Gap Y
       * @see https://tailwindcss.com/docs/gap
       */
      "gap-y": [{
        "gap-y": A()
      }],
      /**
       * Justify Content
       * @see https://tailwindcss.com/docs/justify-content
       */
      "justify-content": [{
        justify: [...wt(), "normal"]
      }],
      /**
       * Justify Items
       * @see https://tailwindcss.com/docs/justify-items
       */
      "justify-items": [{
        "justify-items": [...nt(), "normal"]
      }],
      /**
       * Justify Self
       * @see https://tailwindcss.com/docs/justify-self
       */
      "justify-self": [{
        "justify-self": ["auto", ...nt()]
      }],
      /**
       * Align Content
       * @see https://tailwindcss.com/docs/align-content
       */
      "align-content": [{
        content: ["normal", ...wt()]
      }],
      /**
       * Align Items
       * @see https://tailwindcss.com/docs/align-items
       */
      "align-items": [{
        items: [...nt(), {
          baseline: ["", "last"]
        }]
      }],
      /**
       * Align Self
       * @see https://tailwindcss.com/docs/align-self
       */
      "align-self": [{
        self: ["auto", ...nt(), {
          baseline: ["", "last"]
        }]
      }],
      /**
       * Place Content
       * @see https://tailwindcss.com/docs/place-content
       */
      "place-content": [{
        "place-content": wt()
      }],
      /**
       * Place Items
       * @see https://tailwindcss.com/docs/place-items
       */
      "place-items": [{
        "place-items": [...nt(), "baseline"]
      }],
      /**
       * Place Self
       * @see https://tailwindcss.com/docs/place-self
       */
      "place-self": [{
        "place-self": ["auto", ...nt()]
      }],
      // Spacing
      /**
       * Padding
       * @see https://tailwindcss.com/docs/padding
       */
      p: [{
        p: A()
      }],
      /**
       * Padding Inline
       * @see https://tailwindcss.com/docs/padding
       */
      px: [{
        px: A()
      }],
      /**
       * Padding Block
       * @see https://tailwindcss.com/docs/padding
       */
      py: [{
        py: A()
      }],
      /**
       * Padding Inline Start
       * @see https://tailwindcss.com/docs/padding
       */
      ps: [{
        ps: A()
      }],
      /**
       * Padding Inline End
       * @see https://tailwindcss.com/docs/padding
       */
      pe: [{
        pe: A()
      }],
      /**
       * Padding Block Start
       * @see https://tailwindcss.com/docs/padding
       */
      pbs: [{
        pbs: A()
      }],
      /**
       * Padding Block End
       * @see https://tailwindcss.com/docs/padding
       */
      pbe: [{
        pbe: A()
      }],
      /**
       * Padding Top
       * @see https://tailwindcss.com/docs/padding
       */
      pt: [{
        pt: A()
      }],
      /**
       * Padding Right
       * @see https://tailwindcss.com/docs/padding
       */
      pr: [{
        pr: A()
      }],
      /**
       * Padding Bottom
       * @see https://tailwindcss.com/docs/padding
       */
      pb: [{
        pb: A()
      }],
      /**
       * Padding Left
       * @see https://tailwindcss.com/docs/padding
       */
      pl: [{
        pl: A()
      }],
      /**
       * Margin
       * @see https://tailwindcss.com/docs/margin
       */
      m: [{
        m: ee()
      }],
      /**
       * Margin Inline
       * @see https://tailwindcss.com/docs/margin
       */
      mx: [{
        mx: ee()
      }],
      /**
       * Margin Block
       * @see https://tailwindcss.com/docs/margin
       */
      my: [{
        my: ee()
      }],
      /**
       * Margin Inline Start
       * @see https://tailwindcss.com/docs/margin
       */
      ms: [{
        ms: ee()
      }],
      /**
       * Margin Inline End
       * @see https://tailwindcss.com/docs/margin
       */
      me: [{
        me: ee()
      }],
      /**
       * Margin Block Start
       * @see https://tailwindcss.com/docs/margin
       */
      mbs: [{
        mbs: ee()
      }],
      /**
       * Margin Block End
       * @see https://tailwindcss.com/docs/margin
       */
      mbe: [{
        mbe: ee()
      }],
      /**
       * Margin Top
       * @see https://tailwindcss.com/docs/margin
       */
      mt: [{
        mt: ee()
      }],
      /**
       * Margin Right
       * @see https://tailwindcss.com/docs/margin
       */
      mr: [{
        mr: ee()
      }],
      /**
       * Margin Bottom
       * @see https://tailwindcss.com/docs/margin
       */
      mb: [{
        mb: ee()
      }],
      /**
       * Margin Left
       * @see https://tailwindcss.com/docs/margin
       */
      ml: [{
        ml: ee()
      }],
      /**
       * Space Between X
       * @see https://tailwindcss.com/docs/margin#adding-space-between-children
       */
      "space-x": [{
        "space-x": A()
      }],
      /**
       * Space Between X Reverse
       * @see https://tailwindcss.com/docs/margin#adding-space-between-children
       */
      "space-x-reverse": ["space-x-reverse"],
      /**
       * Space Between Y
       * @see https://tailwindcss.com/docs/margin#adding-space-between-children
       */
      "space-y": [{
        "space-y": A()
      }],
      /**
       * Space Between Y Reverse
       * @see https://tailwindcss.com/docs/margin#adding-space-between-children
       */
      "space-y-reverse": ["space-y-reverse"],
      // --------------
      // --- Sizing ---
      // --------------
      /**
       * Size
       * @see https://tailwindcss.com/docs/width#setting-both-width-and-height
       */
      size: [{
        size: q()
      }],
      /**
       * Inline Size
       * @see https://tailwindcss.com/docs/inline-size
       */
      "inline-size": [{
        inline: ["auto", ...Y()]
      }],
      /**
       * Min-Inline Size
       * @see https://tailwindcss.com/docs/min-inline-size
       */
      "min-inline-size": [{
        "min-inline": ["auto", ...Y()]
      }],
      /**
       * Max-Inline Size
       * @see https://tailwindcss.com/docs/max-inline-size
       */
      "max-inline-size": [{
        "max-inline": ["none", ...Y()]
      }],
      /**
       * Block Size
       * @see https://tailwindcss.com/docs/block-size
       */
      "block-size": [{
        block: ["auto", ...je()]
      }],
      /**
       * Min-Block Size
       * @see https://tailwindcss.com/docs/min-block-size
       */
      "min-block-size": [{
        "min-block": ["auto", ...je()]
      }],
      /**
       * Max-Block Size
       * @see https://tailwindcss.com/docs/max-block-size
       */
      "max-block-size": [{
        "max-block": ["none", ...je()]
      }],
      /**
       * Width
       * @see https://tailwindcss.com/docs/width
       */
      w: [{
        w: [l, "screen", ...q()]
      }],
      /**
       * Min-Width
       * @see https://tailwindcss.com/docs/min-width
       */
      "min-w": [{
        "min-w": [
          l,
          "screen",
          /** Deprecated. @see https://github.com/tailwindlabs/tailwindcss.com/issues/2027#issuecomment-2620152757 */
          "none",
          ...q()
        ]
      }],
      /**
       * Max-Width
       * @see https://tailwindcss.com/docs/max-width
       */
      "max-w": [{
        "max-w": [
          l,
          "screen",
          "none",
          /** Deprecated since Tailwind CSS v4.0.0. @see https://github.com/tailwindlabs/tailwindcss.com/issues/2027#issuecomment-2620152757 */
          "prose",
          /** Deprecated since Tailwind CSS v4.0.0. @see https://github.com/tailwindlabs/tailwindcss.com/issues/2027#issuecomment-2620152757 */
          {
            screen: [i]
          },
          ...q()
        ]
      }],
      /**
       * Height
       * @see https://tailwindcss.com/docs/height
       */
      h: [{
        h: ["screen", "lh", ...q()]
      }],
      /**
       * Min-Height
       * @see https://tailwindcss.com/docs/min-height
       */
      "min-h": [{
        "min-h": ["screen", "lh", "none", ...q()]
      }],
      /**
       * Max-Height
       * @see https://tailwindcss.com/docs/max-height
       */
      "max-h": [{
        "max-h": ["screen", "lh", "none", ...q()]
      }],
      // ------------------
      // --- Typography ---
      // ------------------
      /**
       * Font Size
       * @see https://tailwindcss.com/docs/font-size
       */
      "font-size": [{
        text: ["base", n, Jt, At]
      }],
      /**
       * Font Smoothing
       * @see https://tailwindcss.com/docs/font-smoothing
       */
      "font-smoothing": ["antialiased", "subpixel-antialiased"],
      /**
       * Font Style
       * @see https://tailwindcss.com/docs/font-style
       */
      "font-style": ["italic", "not-italic"],
      /**
       * Font Weight
       * @see https://tailwindcss.com/docs/font-weight
       */
      "font-weight": [{
        font: [s, Ka, Da]
      }],
      /**
       * Font Stretch
       * @see https://tailwindcss.com/docs/font-stretch
       */
      "font-stretch": [{
        "font-stretch": ["ultra-condensed", "extra-condensed", "condensed", "semi-condensed", "normal", "semi-expanded", "expanded", "extra-expanded", "ultra-expanded", us, E]
      }],
      /**
       * Font Family
       * @see https://tailwindcss.com/docs/font-family
       */
      "font-family": [{
        font: [Ba, Ha, t]
      }],
      /**
       * Font Feature Settings
       * @see https://tailwindcss.com/docs/font-feature-settings
       */
      "font-features": [{
        "font-features": [E]
      }],
      /**
       * Font Variant Numeric
       * @see https://tailwindcss.com/docs/font-variant-numeric
       */
      "fvn-normal": ["normal-nums"],
      /**
       * Font Variant Numeric
       * @see https://tailwindcss.com/docs/font-variant-numeric
       */
      "fvn-ordinal": ["ordinal"],
      /**
       * Font Variant Numeric
       * @see https://tailwindcss.com/docs/font-variant-numeric
       */
      "fvn-slashed-zero": ["slashed-zero"],
      /**
       * Font Variant Numeric
       * @see https://tailwindcss.com/docs/font-variant-numeric
       */
      "fvn-figure": ["lining-nums", "oldstyle-nums"],
      /**
       * Font Variant Numeric
       * @see https://tailwindcss.com/docs/font-variant-numeric
       */
      "fvn-spacing": ["proportional-nums", "tabular-nums"],
      /**
       * Font Variant Numeric
       * @see https://tailwindcss.com/docs/font-variant-numeric
       */
      "fvn-fraction": ["diagonal-fractions", "stacked-fractions"],
      /**
       * Letter Spacing
       * @see https://tailwindcss.com/docs/letter-spacing
       */
      tracking: [{
        tracking: [r, P, E]
      }],
      /**
       * Line Clamp
       * @see https://tailwindcss.com/docs/line-clamp
       */
      "line-clamp": [{
        "line-clamp": [V, "none", P, Mr]
      }],
      /**
       * Line Height
       * @see https://tailwindcss.com/docs/line-height
       */
      leading: [{
        leading: [
          "none",
          /** Deprecated since Tailwind CSS v4.0.0. @see https://github.com/tailwindlabs/tailwindcss.com/issues/2027#issuecomment-2620152757 */
          o,
          ...A()
        ]
      }],
      /**
       * List Style Image
       * @see https://tailwindcss.com/docs/list-style-image
       */
      "list-image": [{
        "list-image": ["none", P, E]
      }],
      /**
       * List Style Position
       * @see https://tailwindcss.com/docs/list-style-position
       */
      "list-style-position": [{
        list: ["inside", "outside"]
      }],
      /**
       * List Style Type
       * @see https://tailwindcss.com/docs/list-style-type
       */
      "list-style-type": [{
        list: ["disc", "decimal", "none", P, E]
      }],
      /**
       * Text Alignment
       * @see https://tailwindcss.com/docs/text-align
       */
      "text-alignment": [{
        text: ["left", "center", "right", "justify", "start", "end"]
      }],
      /**
       * Placeholder Color
       * @deprecated since Tailwind CSS v3.0.0
       * @see https://v3.tailwindcss.com/docs/placeholder-color
       */
      "placeholder-color": [{
        placeholder: M()
      }],
      /**
       * Text Color
       * @see https://tailwindcss.com/docs/text-color
       */
      "text-color": [{
        text: M()
      }],
      /**
       * Text Decoration
       * @see https://tailwindcss.com/docs/text-decoration
       */
      "text-decoration": ["underline", "overline", "line-through", "no-underline"],
      /**
       * Text Decoration Style
       * @see https://tailwindcss.com/docs/text-decoration-style
       */
      "text-decoration-style": [{
        decoration: [...st(), "wavy"]
      }],
      /**
       * Text Decoration Thickness
       * @see https://tailwindcss.com/docs/text-decoration-thickness
       */
      "text-decoration-thickness": [{
        decoration: [V, "from-font", "auto", P, At]
      }],
      /**
       * Text Decoration Color
       * @see https://tailwindcss.com/docs/text-decoration-color
       */
      "text-decoration-color": [{
        decoration: M()
      }],
      /**
       * Text Underline Offset
       * @see https://tailwindcss.com/docs/text-underline-offset
       */
      "underline-offset": [{
        "underline-offset": [V, "auto", P, E]
      }],
      /**
       * Text Transform
       * @see https://tailwindcss.com/docs/text-transform
       */
      "text-transform": ["uppercase", "lowercase", "capitalize", "normal-case"],
      /**
       * Text Overflow
       * @see https://tailwindcss.com/docs/text-overflow
       */
      "text-overflow": ["truncate", "text-ellipsis", "text-clip"],
      /**
       * Text Wrap
       * @see https://tailwindcss.com/docs/text-wrap
       */
      "text-wrap": [{
        text: ["wrap", "nowrap", "balance", "pretty"]
      }],
      /**
       * Text Indent
       * @see https://tailwindcss.com/docs/text-indent
       */
      indent: [{
        indent: A()
      }],
      /**
       * Tab Size
       * @see https://tailwindcss.com/docs/tab-size
       */
      "tab-size": [{
        tab: [Ke, P, E]
      }],
      /**
       * Vertical Alignment
       * @see https://tailwindcss.com/docs/vertical-align
       */
      "vertical-align": [{
        align: ["baseline", "top", "middle", "bottom", "text-top", "text-bottom", "sub", "super", P, E]
      }],
      /**
       * Whitespace
       * @see https://tailwindcss.com/docs/whitespace
       */
      whitespace: [{
        whitespace: ["normal", "nowrap", "pre", "pre-line", "pre-wrap", "break-spaces"]
      }],
      /**
       * Word Break
       * @see https://tailwindcss.com/docs/word-break
       */
      break: [{
        break: ["normal", "words", "all", "keep"]
      }],
      /**
       * Overflow Wrap
       * @see https://tailwindcss.com/docs/overflow-wrap
       */
      wrap: [{
        wrap: ["break-word", "anywhere", "normal"]
      }],
      /**
       * Hyphens
       * @see https://tailwindcss.com/docs/hyphens
       */
      hyphens: [{
        hyphens: ["none", "manual", "auto"]
      }],
      /**
       * Content
       * @see https://tailwindcss.com/docs/content
       */
      content: [{
        content: ["none", P, E]
      }],
      // -------------------
      // --- Backgrounds ---
      // -------------------
      /**
       * Background Attachment
       * @see https://tailwindcss.com/docs/background-attachment
       */
      "bg-attachment": [{
        bg: ["fixed", "local", "scroll"]
      }],
      /**
       * Background Clip
       * @see https://tailwindcss.com/docs/background-clip
       */
      "bg-clip": [{
        "bg-clip": ["border", "padding", "content", "text"]
      }],
      /**
       * Background Origin
       * @see https://tailwindcss.com/docs/background-origin
       */
      "bg-origin": [{
        "bg-origin": ["border", "padding", "content"]
      }],
      /**
       * Background Position
       * @see https://tailwindcss.com/docs/background-position
       */
      "bg-position": [{
        bg: We()
      }],
      /**
       * Background Repeat
       * @see https://tailwindcss.com/docs/background-repeat
       */
      "bg-repeat": [{
        bg: Te()
      }],
      /**
       * Background Size
       * @see https://tailwindcss.com/docs/background-size
       */
      "bg-size": [{
        bg: Vt()
      }],
      /**
       * Background Image
       * @see https://tailwindcss.com/docs/background-image
       */
      "bg-image": [{
        bg: ["none", {
          linear: [{
            to: ["t", "tr", "r", "br", "b", "bl", "l", "tl"]
          }, Ke, P, E],
          radial: ["", P, E],
          conic: ["", Ke, P, E]
        }, Ua, Wa]
      }],
      /**
       * Background Color
       * @see https://tailwindcss.com/docs/background-color
       */
      "bg-color": [{
        bg: M()
      }],
      /**
       * Gradient Color Stops From Position
       * @see https://tailwindcss.com/docs/gradient-color-stops
       */
      "gradient-from-pos": [{
        from: Mt()
      }],
      /**
       * Gradient Color Stops Via Position
       * @see https://tailwindcss.com/docs/gradient-color-stops
       */
      "gradient-via-pos": [{
        via: Mt()
      }],
      /**
       * Gradient Color Stops To Position
       * @see https://tailwindcss.com/docs/gradient-color-stops
       */
      "gradient-to-pos": [{
        to: Mt()
      }],
      /**
       * Gradient Color Stops From
       * @see https://tailwindcss.com/docs/gradient-color-stops
       */
      "gradient-from": [{
        from: M()
      }],
      /**
       * Gradient Color Stops Via
       * @see https://tailwindcss.com/docs/gradient-color-stops
       */
      "gradient-via": [{
        via: M()
      }],
      /**
       * Gradient Color Stops To
       * @see https://tailwindcss.com/docs/gradient-color-stops
       */
      "gradient-to": [{
        to: M()
      }],
      // ---------------
      // --- Borders ---
      // ---------------
      /**
       * Border Radius
       * @see https://tailwindcss.com/docs/border-radius
       */
      rounded: [{
        rounded: ge()
      }],
      /**
       * Border Radius Start
       * @see https://tailwindcss.com/docs/border-radius
       */
      "rounded-s": [{
        "rounded-s": ge()
      }],
      /**
       * Border Radius End
       * @see https://tailwindcss.com/docs/border-radius
       */
      "rounded-e": [{
        "rounded-e": ge()
      }],
      /**
       * Border Radius Top
       * @see https://tailwindcss.com/docs/border-radius
       */
      "rounded-t": [{
        "rounded-t": ge()
      }],
      /**
       * Border Radius Right
       * @see https://tailwindcss.com/docs/border-radius
       */
      "rounded-r": [{
        "rounded-r": ge()
      }],
      /**
       * Border Radius Bottom
       * @see https://tailwindcss.com/docs/border-radius
       */
      "rounded-b": [{
        "rounded-b": ge()
      }],
      /**
       * Border Radius Left
       * @see https://tailwindcss.com/docs/border-radius
       */
      "rounded-l": [{
        "rounded-l": ge()
      }],
      /**
       * Border Radius Start Start
       * @see https://tailwindcss.com/docs/border-radius
       */
      "rounded-ss": [{
        "rounded-ss": ge()
      }],
      /**
       * Border Radius Start End
       * @see https://tailwindcss.com/docs/border-radius
       */
      "rounded-se": [{
        "rounded-se": ge()
      }],
      /**
       * Border Radius End End
       * @see https://tailwindcss.com/docs/border-radius
       */
      "rounded-ee": [{
        "rounded-ee": ge()
      }],
      /**
       * Border Radius End Start
       * @see https://tailwindcss.com/docs/border-radius
       */
      "rounded-es": [{
        "rounded-es": ge()
      }],
      /**
       * Border Radius Top Left
       * @see https://tailwindcss.com/docs/border-radius
       */
      "rounded-tl": [{
        "rounded-tl": ge()
      }],
      /**
       * Border Radius Top Right
       * @see https://tailwindcss.com/docs/border-radius
       */
      "rounded-tr": [{
        "rounded-tr": ge()
      }],
      /**
       * Border Radius Bottom Right
       * @see https://tailwindcss.com/docs/border-radius
       */
      "rounded-br": [{
        "rounded-br": ge()
      }],
      /**
       * Border Radius Bottom Left
       * @see https://tailwindcss.com/docs/border-radius
       */
      "rounded-bl": [{
        "rounded-bl": ge()
      }],
      /**
       * Border Width
       * @see https://tailwindcss.com/docs/border-width
       */
      "border-w": [{
        border: ae()
      }],
      /**
       * Border Width Inline
       * @see https://tailwindcss.com/docs/border-width
       */
      "border-w-x": [{
        "border-x": ae()
      }],
      /**
       * Border Width Block
       * @see https://tailwindcss.com/docs/border-width
       */
      "border-w-y": [{
        "border-y": ae()
      }],
      /**
       * Border Width Inline Start
       * @see https://tailwindcss.com/docs/border-width
       */
      "border-w-s": [{
        "border-s": ae()
      }],
      /**
       * Border Width Inline End
       * @see https://tailwindcss.com/docs/border-width
       */
      "border-w-e": [{
        "border-e": ae()
      }],
      /**
       * Border Width Block Start
       * @see https://tailwindcss.com/docs/border-width
       */
      "border-w-bs": [{
        "border-bs": ae()
      }],
      /**
       * Border Width Block End
       * @see https://tailwindcss.com/docs/border-width
       */
      "border-w-be": [{
        "border-be": ae()
      }],
      /**
       * Border Width Top
       * @see https://tailwindcss.com/docs/border-width
       */
      "border-w-t": [{
        "border-t": ae()
      }],
      /**
       * Border Width Right
       * @see https://tailwindcss.com/docs/border-width
       */
      "border-w-r": [{
        "border-r": ae()
      }],
      /**
       * Border Width Bottom
       * @see https://tailwindcss.com/docs/border-width
       */
      "border-w-b": [{
        "border-b": ae()
      }],
      /**
       * Border Width Left
       * @see https://tailwindcss.com/docs/border-width
       */
      "border-w-l": [{
        "border-l": ae()
      }],
      /**
       * Divide Width X
       * @see https://tailwindcss.com/docs/border-width#between-children
       */
      "divide-x": [{
        "divide-x": ae()
      }],
      /**
       * Divide Width X Reverse
       * @see https://tailwindcss.com/docs/border-width#between-children
       */
      "divide-x-reverse": ["divide-x-reverse"],
      /**
       * Divide Width Y
       * @see https://tailwindcss.com/docs/border-width#between-children
       */
      "divide-y": [{
        "divide-y": ae()
      }],
      /**
       * Divide Width Y Reverse
       * @see https://tailwindcss.com/docs/border-width#between-children
       */
      "divide-y-reverse": ["divide-y-reverse"],
      /**
       * Border Style
       * @see https://tailwindcss.com/docs/border-style
       */
      "border-style": [{
        border: [...st(), "hidden", "none"]
      }],
      /**
       * Divide Style
       * @see https://tailwindcss.com/docs/border-style#setting-the-divider-style
       */
      "divide-style": [{
        divide: [...st(), "hidden", "none"]
      }],
      /**
       * Border Color
       * @see https://tailwindcss.com/docs/border-color
       */
      "border-color": [{
        border: M()
      }],
      /**
       * Border Color Inline
       * @see https://tailwindcss.com/docs/border-color
       */
      "border-color-x": [{
        "border-x": M()
      }],
      /**
       * Border Color Block
       * @see https://tailwindcss.com/docs/border-color
       */
      "border-color-y": [{
        "border-y": M()
      }],
      /**
       * Border Color Inline Start
       * @see https://tailwindcss.com/docs/border-color
       */
      "border-color-s": [{
        "border-s": M()
      }],
      /**
       * Border Color Inline End
       * @see https://tailwindcss.com/docs/border-color
       */
      "border-color-e": [{
        "border-e": M()
      }],
      /**
       * Border Color Block Start
       * @see https://tailwindcss.com/docs/border-color
       */
      "border-color-bs": [{
        "border-bs": M()
      }],
      /**
       * Border Color Block End
       * @see https://tailwindcss.com/docs/border-color
       */
      "border-color-be": [{
        "border-be": M()
      }],
      /**
       * Border Color Top
       * @see https://tailwindcss.com/docs/border-color
       */
      "border-color-t": [{
        "border-t": M()
      }],
      /**
       * Border Color Right
       * @see https://tailwindcss.com/docs/border-color
       */
      "border-color-r": [{
        "border-r": M()
      }],
      /**
       * Border Color Bottom
       * @see https://tailwindcss.com/docs/border-color
       */
      "border-color-b": [{
        "border-b": M()
      }],
      /**
       * Border Color Left
       * @see https://tailwindcss.com/docs/border-color
       */
      "border-color-l": [{
        "border-l": M()
      }],
      /**
       * Divide Color
       * @see https://tailwindcss.com/docs/divide-color
       */
      "divide-color": [{
        divide: M()
      }],
      /**
       * Outline Style
       * @see https://tailwindcss.com/docs/outline-style
       */
      "outline-style": [{
        outline: [...st(), "none", "hidden"]
      }],
      /**
       * Outline Offset
       * @see https://tailwindcss.com/docs/outline-offset
       */
      "outline-offset": [{
        "outline-offset": [V, P, E]
      }],
      /**
       * Outline Width
       * @see https://tailwindcss.com/docs/outline-width
       */
      "outline-w": [{
        outline: ["", V, Jt, At]
      }],
      /**
       * Outline Color
       * @see https://tailwindcss.com/docs/outline-color
       */
      "outline-color": [{
        outline: M()
      }],
      // ---------------
      // --- Effects ---
      // ---------------
      /**
       * Box Shadow
       * @see https://tailwindcss.com/docs/box-shadow
       */
      shadow: [{
        shadow: [
          // Deprecated since Tailwind CSS v4.0.0
          "",
          // Deprecated since Tailwind CSS v4.0.0
          "inner",
          "none",
          f,
          Sn,
          kn
        ]
      }],
      /**
       * Box Shadow Color
       * @see https://tailwindcss.com/docs/box-shadow#setting-the-shadow-color
       */
      "shadow-color": [{
        shadow: M()
      }],
      /**
       * Inset Box Shadow
       * @see https://tailwindcss.com/docs/box-shadow#adding-an-inset-shadow
       */
      "inset-shadow": [{
        "inset-shadow": ["none", h, Sn, kn]
      }],
      /**
       * Inset Box Shadow Color
       * @see https://tailwindcss.com/docs/box-shadow#setting-the-inset-shadow-color
       */
      "inset-shadow-color": [{
        "inset-shadow": M()
      }],
      /**
       * Ring Width
       * @see https://tailwindcss.com/docs/box-shadow#adding-a-ring
       */
      "ring-w": [{
        ring: ae()
      }],
      /**
       * Ring Width Inset
       * @see https://v3.tailwindcss.com/docs/ring-width#inset-rings
       * @deprecated since Tailwind CSS v4.0.0
       * @see https://github.com/tailwindlabs/tailwindcss/blob/v4.0.0/packages/tailwindcss/src/utilities.ts#L4158
       */
      "ring-w-inset": ["ring-inset"],
      /**
       * Ring Color
       * @see https://tailwindcss.com/docs/box-shadow#setting-the-ring-color
       */
      "ring-color": [{
        ring: M()
      }],
      /**
       * Ring Offset Width
       * @see https://v3.tailwindcss.com/docs/ring-offset-width
       * @deprecated since Tailwind CSS v4.0.0
       * @see https://github.com/tailwindlabs/tailwindcss/blob/v4.0.0/packages/tailwindcss/src/utilities.ts#L4158
       */
      "ring-offset-w": [{
        "ring-offset": [V, At]
      }],
      /**
       * Ring Offset Color
       * @see https://v3.tailwindcss.com/docs/ring-offset-color
       * @deprecated since Tailwind CSS v4.0.0
       * @see https://github.com/tailwindlabs/tailwindcss/blob/v4.0.0/packages/tailwindcss/src/utilities.ts#L4158
       */
      "ring-offset-color": [{
        "ring-offset": M()
      }],
      /**
       * Inset Ring Width
       * @see https://tailwindcss.com/docs/box-shadow#adding-an-inset-ring
       */
      "inset-ring-w": [{
        "inset-ring": ae()
      }],
      /**
       * Inset Ring Color
       * @see https://tailwindcss.com/docs/box-shadow#setting-the-inset-ring-color
       */
      "inset-ring-color": [{
        "inset-ring": M()
      }],
      /**
       * Text Shadow
       * @see https://tailwindcss.com/docs/text-shadow
       */
      "text-shadow": [{
        "text-shadow": ["none", b, Sn, kn]
      }],
      /**
       * Text Shadow Color
       * @see https://tailwindcss.com/docs/text-shadow#setting-the-shadow-color
       */
      "text-shadow-color": [{
        "text-shadow": M()
      }],
      /**
       * Opacity
       * @see https://tailwindcss.com/docs/opacity
       */
      opacity: [{
        opacity: [V, P, E]
      }],
      /**
       * Mix Blend Mode
       * @see https://tailwindcss.com/docs/mix-blend-mode
       */
      "mix-blend": [{
        "mix-blend": [...Ut(), "plus-darker", "plus-lighter"]
      }],
      /**
       * Background Blend Mode
       * @see https://tailwindcss.com/docs/background-blend-mode
       */
      "bg-blend": [{
        "bg-blend": Ut()
      }],
      /**
       * Mask Clip
       * @see https://tailwindcss.com/docs/mask-clip
       */
      "mask-clip": [{
        "mask-clip": ["border", "padding", "content", "fill", "stroke", "view"]
      }, "mask-no-clip"],
      /**
       * Mask Composite
       * @see https://tailwindcss.com/docs/mask-composite
       */
      "mask-composite": [{
        mask: ["add", "subtract", "intersect", "exclude"]
      }],
      /**
       * Mask Image
       * @see https://tailwindcss.com/docs/mask-image
       */
      "mask-image-linear-pos": [{
        "mask-linear": [V]
      }],
      "mask-image-linear-from-pos": [{
        "mask-linear-from": ie()
      }],
      "mask-image-linear-to-pos": [{
        "mask-linear-to": ie()
      }],
      "mask-image-linear-from-color": [{
        "mask-linear-from": M()
      }],
      "mask-image-linear-to-color": [{
        "mask-linear-to": M()
      }],
      "mask-image-t-from-pos": [{
        "mask-t-from": ie()
      }],
      "mask-image-t-to-pos": [{
        "mask-t-to": ie()
      }],
      "mask-image-t-from-color": [{
        "mask-t-from": M()
      }],
      "mask-image-t-to-color": [{
        "mask-t-to": M()
      }],
      "mask-image-r-from-pos": [{
        "mask-r-from": ie()
      }],
      "mask-image-r-to-pos": [{
        "mask-r-to": ie()
      }],
      "mask-image-r-from-color": [{
        "mask-r-from": M()
      }],
      "mask-image-r-to-color": [{
        "mask-r-to": M()
      }],
      "mask-image-b-from-pos": [{
        "mask-b-from": ie()
      }],
      "mask-image-b-to-pos": [{
        "mask-b-to": ie()
      }],
      "mask-image-b-from-color": [{
        "mask-b-from": M()
      }],
      "mask-image-b-to-color": [{
        "mask-b-to": M()
      }],
      "mask-image-l-from-pos": [{
        "mask-l-from": ie()
      }],
      "mask-image-l-to-pos": [{
        "mask-l-to": ie()
      }],
      "mask-image-l-from-color": [{
        "mask-l-from": M()
      }],
      "mask-image-l-to-color": [{
        "mask-l-to": M()
      }],
      "mask-image-x-from-pos": [{
        "mask-x-from": ie()
      }],
      "mask-image-x-to-pos": [{
        "mask-x-to": ie()
      }],
      "mask-image-x-from-color": [{
        "mask-x-from": M()
      }],
      "mask-image-x-to-color": [{
        "mask-x-to": M()
      }],
      "mask-image-y-from-pos": [{
        "mask-y-from": ie()
      }],
      "mask-image-y-to-pos": [{
        "mask-y-to": ie()
      }],
      "mask-image-y-from-color": [{
        "mask-y-from": M()
      }],
      "mask-image-y-to-color": [{
        "mask-y-to": M()
      }],
      "mask-image-radial": [{
        "mask-radial": [P, E]
      }],
      "mask-image-radial-from-pos": [{
        "mask-radial-from": ie()
      }],
      "mask-image-radial-to-pos": [{
        "mask-radial-to": ie()
      }],
      "mask-image-radial-from-color": [{
        "mask-radial-from": M()
      }],
      "mask-image-radial-to-color": [{
        "mask-radial-to": M()
      }],
      "mask-image-radial-shape": [{
        "mask-radial": ["circle", "ellipse"]
      }],
      "mask-image-radial-size": [{
        "mask-radial": [{
          closest: ["side", "corner"],
          farthest: ["side", "corner"]
        }]
      }],
      "mask-image-radial-pos": [{
        "mask-radial-at": O()
      }],
      "mask-image-conic-pos": [{
        "mask-conic": [V]
      }],
      "mask-image-conic-from-pos": [{
        "mask-conic-from": ie()
      }],
      "mask-image-conic-to-pos": [{
        "mask-conic-to": ie()
      }],
      "mask-image-conic-from-color": [{
        "mask-conic-from": M()
      }],
      "mask-image-conic-to-color": [{
        "mask-conic-to": M()
      }],
      /**
       * Mask Mode
       * @see https://tailwindcss.com/docs/mask-mode
       */
      "mask-mode": [{
        mask: ["alpha", "luminance", "match"]
      }],
      /**
       * Mask Origin
       * @see https://tailwindcss.com/docs/mask-origin
       */
      "mask-origin": [{
        "mask-origin": ["border", "padding", "content", "fill", "stroke", "view"]
      }],
      /**
       * Mask Position
       * @see https://tailwindcss.com/docs/mask-position
       */
      "mask-position": [{
        mask: We()
      }],
      /**
       * Mask Repeat
       * @see https://tailwindcss.com/docs/mask-repeat
       */
      "mask-repeat": [{
        mask: Te()
      }],
      /**
       * Mask Size
       * @see https://tailwindcss.com/docs/mask-size
       */
      "mask-size": [{
        mask: Vt()
      }],
      /**
       * Mask Type
       * @see https://tailwindcss.com/docs/mask-type
       */
      "mask-type": [{
        "mask-type": ["alpha", "luminance"]
      }],
      /**
       * Mask Image
       * @see https://tailwindcss.com/docs/mask-image
       */
      "mask-image": [{
        mask: ["none", P, E]
      }],
      // ---------------
      // --- Filters ---
      // ---------------
      /**
       * Filter
       * @see https://tailwindcss.com/docs/filter
       */
      filter: [{
        filter: [
          // Deprecated since Tailwind CSS v3.0.0
          "",
          "none",
          P,
          E
        ]
      }],
      /**
       * Blur
       * @see https://tailwindcss.com/docs/blur
       */
      blur: [{
        blur: qn()
      }],
      /**
       * Brightness
       * @see https://tailwindcss.com/docs/brightness
       */
      brightness: [{
        brightness: [V, P, E]
      }],
      /**
       * Contrast
       * @see https://tailwindcss.com/docs/contrast
       */
      contrast: [{
        contrast: [V, P, E]
      }],
      /**
       * Drop Shadow
       * @see https://tailwindcss.com/docs/drop-shadow
       */
      "drop-shadow": [{
        "drop-shadow": [
          // Deprecated since Tailwind CSS v4.0.0
          "",
          "none",
          w,
          Sn,
          kn
        ]
      }],
      /**
       * Drop Shadow Color
       * @see https://tailwindcss.com/docs/filter-drop-shadow#setting-the-shadow-color
       */
      "drop-shadow-color": [{
        "drop-shadow": M()
      }],
      /**
       * Grayscale
       * @see https://tailwindcss.com/docs/grayscale
       */
      grayscale: [{
        grayscale: ["", V, P, E]
      }],
      /**
       * Hue Rotate
       * @see https://tailwindcss.com/docs/hue-rotate
       */
      "hue-rotate": [{
        "hue-rotate": [V, P, E]
      }],
      /**
       * Invert
       * @see https://tailwindcss.com/docs/invert
       */
      invert: [{
        invert: ["", V, P, E]
      }],
      /**
       * Saturate
       * @see https://tailwindcss.com/docs/saturate
       */
      saturate: [{
        saturate: [V, P, E]
      }],
      /**
       * Sepia
       * @see https://tailwindcss.com/docs/sepia
       */
      sepia: [{
        sepia: ["", V, P, E]
      }],
      /**
       * Backdrop Filter
       * @see https://tailwindcss.com/docs/backdrop-filter
       */
      "backdrop-filter": [{
        "backdrop-filter": [
          // Deprecated since Tailwind CSS v3.0.0
          "",
          "none",
          P,
          E
        ]
      }],
      /**
       * Backdrop Blur
       * @see https://tailwindcss.com/docs/backdrop-blur
       */
      "backdrop-blur": [{
        "backdrop-blur": qn()
      }],
      /**
       * Backdrop Brightness
       * @see https://tailwindcss.com/docs/backdrop-brightness
       */
      "backdrop-brightness": [{
        "backdrop-brightness": [V, P, E]
      }],
      /**
       * Backdrop Contrast
       * @see https://tailwindcss.com/docs/backdrop-contrast
       */
      "backdrop-contrast": [{
        "backdrop-contrast": [V, P, E]
      }],
      /**
       * Backdrop Grayscale
       * @see https://tailwindcss.com/docs/backdrop-grayscale
       */
      "backdrop-grayscale": [{
        "backdrop-grayscale": ["", V, P, E]
      }],
      /**
       * Backdrop Hue Rotate
       * @see https://tailwindcss.com/docs/backdrop-hue-rotate
       */
      "backdrop-hue-rotate": [{
        "backdrop-hue-rotate": [V, P, E]
      }],
      /**
       * Backdrop Invert
       * @see https://tailwindcss.com/docs/backdrop-invert
       */
      "backdrop-invert": [{
        "backdrop-invert": ["", V, P, E]
      }],
      /**
       * Backdrop Opacity
       * @see https://tailwindcss.com/docs/backdrop-opacity
       */
      "backdrop-opacity": [{
        "backdrop-opacity": [V, P, E]
      }],
      /**
       * Backdrop Saturate
       * @see https://tailwindcss.com/docs/backdrop-saturate
       */
      "backdrop-saturate": [{
        "backdrop-saturate": [V, P, E]
      }],
      /**
       * Backdrop Sepia
       * @see https://tailwindcss.com/docs/backdrop-sepia
       */
      "backdrop-sepia": [{
        "backdrop-sepia": ["", V, P, E]
      }],
      // --------------
      // --- Tables ---
      // --------------
      /**
       * Border Collapse
       * @see https://tailwindcss.com/docs/border-collapse
       */
      "border-collapse": [{
        border: ["collapse", "separate"]
      }],
      /**
       * Border Spacing
       * @see https://tailwindcss.com/docs/border-spacing
       */
      "border-spacing": [{
        "border-spacing": A()
      }],
      /**
       * Border Spacing X
       * @see https://tailwindcss.com/docs/border-spacing
       */
      "border-spacing-x": [{
        "border-spacing-x": A()
      }],
      /**
       * Border Spacing Y
       * @see https://tailwindcss.com/docs/border-spacing
       */
      "border-spacing-y": [{
        "border-spacing-y": A()
      }],
      /**
       * Table Layout
       * @see https://tailwindcss.com/docs/table-layout
       */
      "table-layout": [{
        table: ["auto", "fixed"]
      }],
      /**
       * Caption Side
       * @see https://tailwindcss.com/docs/caption-side
       */
      caption: [{
        caption: ["top", "bottom"]
      }],
      // ---------------------------------
      // --- Transitions and Animation ---
      // ---------------------------------
      /**
       * Transition Property
       * @see https://tailwindcss.com/docs/transition-property
       */
      transition: [{
        transition: ["", "all", "colors", "opacity", "shadow", "transform", "none", P, E]
      }],
      /**
       * Transition Behavior
       * @see https://tailwindcss.com/docs/transition-behavior
       */
      "transition-behavior": [{
        transition: ["normal", "discrete"]
      }],
      /**
       * Transition Duration
       * @see https://tailwindcss.com/docs/transition-duration
       */
      duration: [{
        duration: [V, "initial", P, E]
      }],
      /**
       * Transition Timing Function
       * @see https://tailwindcss.com/docs/transition-timing-function
       */
      ease: [{
        ease: ["linear", "initial", D, P, E]
      }],
      /**
       * Transition Delay
       * @see https://tailwindcss.com/docs/transition-delay
       */
      delay: [{
        delay: [V, P, E]
      }],
      /**
       * Animation
       * @see https://tailwindcss.com/docs/animation
       */
      animate: [{
        animate: ["none", j, P, E]
      }],
      // ------------------
      // --- Transforms ---
      // ------------------
      /**
       * Backface Visibility
       * @see https://tailwindcss.com/docs/backface-visibility
       */
      backface: [{
        backface: ["hidden", "visible"]
      }],
      /**
       * Perspective
       * @see https://tailwindcss.com/docs/perspective
       */
      perspective: [{
        perspective: [C, P, E]
      }],
      /**
       * Perspective Origin
       * @see https://tailwindcss.com/docs/perspective-origin
       */
      "perspective-origin": [{
        "perspective-origin": B()
      }],
      /**
       * Rotate
       * @see https://tailwindcss.com/docs/rotate
       */
      rotate: [{
        rotate: a()
      }],
      /**
       * Rotate X
       * @see https://tailwindcss.com/docs/rotate
       */
      "rotate-x": [{
        "rotate-x": a()
      }],
      /**
       * Rotate Y
       * @see https://tailwindcss.com/docs/rotate
       */
      "rotate-y": [{
        "rotate-y": a()
      }],
      /**
       * Rotate Z
       * @see https://tailwindcss.com/docs/rotate
       */
      "rotate-z": [{
        "rotate-z": a()
      }],
      /**
       * Scale
       * @see https://tailwindcss.com/docs/scale
       */
      scale: [{
        scale: d()
      }],
      /**
       * Scale X
       * @see https://tailwindcss.com/docs/scale
       */
      "scale-x": [{
        "scale-x": d()
      }],
      /**
       * Scale Y
       * @see https://tailwindcss.com/docs/scale
       */
      "scale-y": [{
        "scale-y": d()
      }],
      /**
       * Scale Z
       * @see https://tailwindcss.com/docs/scale
       */
      "scale-z": [{
        "scale-z": d()
      }],
      /**
       * Scale 3D
       * @see https://tailwindcss.com/docs/scale
       */
      "scale-3d": ["scale-3d"],
      /**
       * Skew
       * @see https://tailwindcss.com/docs/skew
       */
      skew: [{
        skew: p()
      }],
      /**
       * Skew X
       * @see https://tailwindcss.com/docs/skew
       */
      "skew-x": [{
        "skew-x": p()
      }],
      /**
       * Skew Y
       * @see https://tailwindcss.com/docs/skew
       */
      "skew-y": [{
        "skew-y": p()
      }],
      /**
       * Transform
       * @see https://tailwindcss.com/docs/transform
       */
      transform: [{
        transform: [P, E, "", "none", "gpu", "cpu"]
      }],
      /**
       * Transform Origin
       * @see https://tailwindcss.com/docs/transform-origin
       */
      "transform-origin": [{
        origin: B()
      }],
      /**
       * Transform Style
       * @see https://tailwindcss.com/docs/transform-style
       */
      "transform-style": [{
        transform: ["3d", "flat"]
      }],
      /**
       * Translate
       * @see https://tailwindcss.com/docs/translate
       */
      translate: [{
        translate: x()
      }],
      /**
       * Translate X
       * @see https://tailwindcss.com/docs/translate
       */
      "translate-x": [{
        "translate-x": x()
      }],
      /**
       * Translate Y
       * @see https://tailwindcss.com/docs/translate
       */
      "translate-y": [{
        "translate-y": x()
      }],
      /**
       * Translate Z
       * @see https://tailwindcss.com/docs/translate
       */
      "translate-z": [{
        "translate-z": x()
      }],
      /**
       * Translate None
       * @see https://tailwindcss.com/docs/translate
       */
      "translate-none": ["translate-none"],
      /**
       * Zoom
       * @see https://tailwindcss.com/docs/zoom
       */
      zoom: [{
        zoom: [Ke, P, E]
      }],
      // ---------------------
      // --- Interactivity ---
      // ---------------------
      /**
       * Accent Color
       * @see https://tailwindcss.com/docs/accent-color
       */
      accent: [{
        accent: M()
      }],
      /**
       * Appearance
       * @see https://tailwindcss.com/docs/appearance
       */
      appearance: [{
        appearance: ["none", "auto"]
      }],
      /**
       * Caret Color
       * @see https://tailwindcss.com/docs/just-in-time-mode#caret-color-utilities
       */
      "caret-color": [{
        caret: M()
      }],
      /**
       * Color Scheme
       * @see https://tailwindcss.com/docs/color-scheme
       */
      "color-scheme": [{
        scheme: ["normal", "dark", "light", "light-dark", "only-dark", "only-light"]
      }],
      /**
       * Cursor
       * @see https://tailwindcss.com/docs/cursor
       */
      cursor: [{
        cursor: ["auto", "default", "pointer", "wait", "text", "move", "help", "not-allowed", "none", "context-menu", "progress", "cell", "crosshair", "vertical-text", "alias", "copy", "no-drop", "grab", "grabbing", "all-scroll", "col-resize", "row-resize", "n-resize", "e-resize", "s-resize", "w-resize", "ne-resize", "nw-resize", "se-resize", "sw-resize", "ew-resize", "ns-resize", "nesw-resize", "nwse-resize", "zoom-in", "zoom-out", P, E]
      }],
      /**
       * Field Sizing
       * @see https://tailwindcss.com/docs/field-sizing
       */
      "field-sizing": [{
        "field-sizing": ["fixed", "content"]
      }],
      /**
       * Pointer Events
       * @see https://tailwindcss.com/docs/pointer-events
       */
      "pointer-events": [{
        "pointer-events": ["auto", "none"]
      }],
      /**
       * Resize
       * @see https://tailwindcss.com/docs/resize
       */
      resize: [{
        resize: ["none", "", "y", "x"]
      }],
      /**
       * Scroll Behavior
       * @see https://tailwindcss.com/docs/scroll-behavior
       */
      "scroll-behavior": [{
        scroll: ["auto", "smooth"]
      }],
      /**
       * Scrollbar Thumb Color
       * @see https://tailwindcss.com/docs/scrollbar-color
       */
      "scrollbar-thumb-color": [{
        "scrollbar-thumb": M()
      }],
      /**
       * Scrollbar Track Color
       * @see https://tailwindcss.com/docs/scrollbar-color
       */
      "scrollbar-track-color": [{
        "scrollbar-track": M()
      }],
      /**
       * Scrollbar Gutter
       * @see https://tailwindcss.com/docs/scrollbar-gutter
       */
      "scrollbar-gutter": [{
        "scrollbar-gutter": ["auto", "stable", "both"]
      }],
      /**
       * Scrollbar Width
       * @see https://tailwindcss.com/docs/scrollbar-width
       */
      "scrollbar-w": [{
        scrollbar: ["auto", "thin", "none"]
      }],
      /**
       * Scroll Margin
       * @see https://tailwindcss.com/docs/scroll-margin
       */
      "scroll-m": [{
        "scroll-m": A()
      }],
      /**
       * Scroll Margin Inline
       * @see https://tailwindcss.com/docs/scroll-margin
       */
      "scroll-mx": [{
        "scroll-mx": A()
      }],
      /**
       * Scroll Margin Block
       * @see https://tailwindcss.com/docs/scroll-margin
       */
      "scroll-my": [{
        "scroll-my": A()
      }],
      /**
       * Scroll Margin Inline Start
       * @see https://tailwindcss.com/docs/scroll-margin
       */
      "scroll-ms": [{
        "scroll-ms": A()
      }],
      /**
       * Scroll Margin Inline End
       * @see https://tailwindcss.com/docs/scroll-margin
       */
      "scroll-me": [{
        "scroll-me": A()
      }],
      /**
       * Scroll Margin Block Start
       * @see https://tailwindcss.com/docs/scroll-margin
       */
      "scroll-mbs": [{
        "scroll-mbs": A()
      }],
      /**
       * Scroll Margin Block End
       * @see https://tailwindcss.com/docs/scroll-margin
       */
      "scroll-mbe": [{
        "scroll-mbe": A()
      }],
      /**
       * Scroll Margin Top
       * @see https://tailwindcss.com/docs/scroll-margin
       */
      "scroll-mt": [{
        "scroll-mt": A()
      }],
      /**
       * Scroll Margin Right
       * @see https://tailwindcss.com/docs/scroll-margin
       */
      "scroll-mr": [{
        "scroll-mr": A()
      }],
      /**
       * Scroll Margin Bottom
       * @see https://tailwindcss.com/docs/scroll-margin
       */
      "scroll-mb": [{
        "scroll-mb": A()
      }],
      /**
       * Scroll Margin Left
       * @see https://tailwindcss.com/docs/scroll-margin
       */
      "scroll-ml": [{
        "scroll-ml": A()
      }],
      /**
       * Scroll Padding
       * @see https://tailwindcss.com/docs/scroll-padding
       */
      "scroll-p": [{
        "scroll-p": A()
      }],
      /**
       * Scroll Padding Inline
       * @see https://tailwindcss.com/docs/scroll-padding
       */
      "scroll-px": [{
        "scroll-px": A()
      }],
      /**
       * Scroll Padding Block
       * @see https://tailwindcss.com/docs/scroll-padding
       */
      "scroll-py": [{
        "scroll-py": A()
      }],
      /**
       * Scroll Padding Inline Start
       * @see https://tailwindcss.com/docs/scroll-padding
       */
      "scroll-ps": [{
        "scroll-ps": A()
      }],
      /**
       * Scroll Padding Inline End
       * @see https://tailwindcss.com/docs/scroll-padding
       */
      "scroll-pe": [{
        "scroll-pe": A()
      }],
      /**
       * Scroll Padding Block Start
       * @see https://tailwindcss.com/docs/scroll-padding
       */
      "scroll-pbs": [{
        "scroll-pbs": A()
      }],
      /**
       * Scroll Padding Block End
       * @see https://tailwindcss.com/docs/scroll-padding
       */
      "scroll-pbe": [{
        "scroll-pbe": A()
      }],
      /**
       * Scroll Padding Top
       * @see https://tailwindcss.com/docs/scroll-padding
       */
      "scroll-pt": [{
        "scroll-pt": A()
      }],
      /**
       * Scroll Padding Right
       * @see https://tailwindcss.com/docs/scroll-padding
       */
      "scroll-pr": [{
        "scroll-pr": A()
      }],
      /**
       * Scroll Padding Bottom
       * @see https://tailwindcss.com/docs/scroll-padding
       */
      "scroll-pb": [{
        "scroll-pb": A()
      }],
      /**
       * Scroll Padding Left
       * @see https://tailwindcss.com/docs/scroll-padding
       */
      "scroll-pl": [{
        "scroll-pl": A()
      }],
      /**
       * Scroll Snap Align
       * @see https://tailwindcss.com/docs/scroll-snap-align
       */
      "snap-align": [{
        snap: ["start", "end", "center", "align-none"]
      }],
      /**
       * Scroll Snap Stop
       * @see https://tailwindcss.com/docs/scroll-snap-stop
       */
      "snap-stop": [{
        snap: ["normal", "always"]
      }],
      /**
       * Scroll Snap Type
       * @see https://tailwindcss.com/docs/scroll-snap-type
       */
      "snap-type": [{
        snap: ["none", "x", "y", "both"]
      }],
      /**
       * Scroll Snap Type Strictness
       * @see https://tailwindcss.com/docs/scroll-snap-type
       */
      "snap-strictness": [{
        snap: ["mandatory", "proximity"]
      }],
      /**
       * Touch Action
       * @see https://tailwindcss.com/docs/touch-action
       */
      touch: [{
        touch: ["auto", "none", "manipulation"]
      }],
      /**
       * Touch Action X
       * @see https://tailwindcss.com/docs/touch-action
       */
      "touch-x": [{
        "touch-pan": ["x", "left", "right"]
      }],
      /**
       * Touch Action Y
       * @see https://tailwindcss.com/docs/touch-action
       */
      "touch-y": [{
        "touch-pan": ["y", "up", "down"]
      }],
      /**
       * Touch Action Pinch Zoom
       * @see https://tailwindcss.com/docs/touch-action
       */
      "touch-pz": ["touch-pinch-zoom"],
      /**
       * User Select
       * @see https://tailwindcss.com/docs/user-select
       */
      select: [{
        select: ["none", "text", "all", "auto"]
      }],
      /**
       * Will Change
       * @see https://tailwindcss.com/docs/will-change
       */
      "will-change": [{
        "will-change": ["auto", "scroll", "contents", "transform", P, E]
      }],
      // -----------
      // --- SVG ---
      // -----------
      /**
       * Fill
       * @see https://tailwindcss.com/docs/fill
       */
      fill: [{
        fill: ["none", ...M()]
      }],
      /**
       * Stroke Width
       * @see https://tailwindcss.com/docs/stroke-width
       */
      "stroke-w": [{
        stroke: [V, Jt, At, Mr]
      }],
      /**
       * Stroke
       * @see https://tailwindcss.com/docs/stroke
       */
      stroke: [{
        stroke: ["none", ...M()]
      }],
      // ---------------------
      // --- Accessibility ---
      // ---------------------
      /**
       * Forced Color Adjust
       * @see https://tailwindcss.com/docs/forced-color-adjust
       */
      "forced-color-adjust": [{
        "forced-color-adjust": ["auto", "none"]
      }]
    },
    conflictingClassGroups: {
      "container-named": ["container-type"],
      overflow: ["overflow-x", "overflow-y"],
      overscroll: ["overscroll-x", "overscroll-y"],
      inset: ["inset-x", "inset-y", "inset-bs", "inset-be", "start", "end", "top", "right", "bottom", "left"],
      "inset-x": ["start", "end", "right", "left"],
      "inset-y": ["inset-bs", "inset-be", "top", "bottom"],
      flex: ["basis", "grow", "shrink"],
      gap: ["gap-x", "gap-y"],
      p: ["px", "py", "ps", "pe", "pbs", "pbe", "pt", "pr", "pb", "pl"],
      px: ["ps", "pe", "pr", "pl"],
      py: ["pbs", "pbe", "pt", "pb"],
      m: ["mx", "my", "ms", "me", "mbs", "mbe", "mt", "mr", "mb", "ml"],
      mx: ["ms", "me", "mr", "ml"],
      my: ["mbs", "mbe", "mt", "mb"],
      size: ["w", "h"],
      "font-size": ["leading"],
      "fvn-normal": ["fvn-ordinal", "fvn-slashed-zero", "fvn-figure", "fvn-spacing", "fvn-fraction"],
      "fvn-ordinal": ["fvn-normal"],
      "fvn-slashed-zero": ["fvn-normal"],
      "fvn-figure": ["fvn-normal"],
      "fvn-spacing": ["fvn-normal"],
      "fvn-fraction": ["fvn-normal"],
      "line-clamp": ["display", "overflow"],
      rounded: ["rounded-s", "rounded-e", "rounded-t", "rounded-r", "rounded-b", "rounded-l", "rounded-ss", "rounded-se", "rounded-ee", "rounded-es", "rounded-tl", "rounded-tr", "rounded-br", "rounded-bl"],
      "rounded-s": ["rounded-ss", "rounded-es"],
      "rounded-e": ["rounded-se", "rounded-ee"],
      "rounded-t": ["rounded-tl", "rounded-tr"],
      "rounded-r": ["rounded-tr", "rounded-br"],
      "rounded-b": ["rounded-br", "rounded-bl"],
      "rounded-l": ["rounded-tl", "rounded-bl"],
      "border-spacing": ["border-spacing-x", "border-spacing-y"],
      "border-w": ["border-w-x", "border-w-y", "border-w-s", "border-w-e", "border-w-bs", "border-w-be", "border-w-t", "border-w-r", "border-w-b", "border-w-l"],
      "border-w-x": ["border-w-s", "border-w-e", "border-w-r", "border-w-l"],
      "border-w-y": ["border-w-bs", "border-w-be", "border-w-t", "border-w-b"],
      "border-color": ["border-color-x", "border-color-y", "border-color-s", "border-color-e", "border-color-bs", "border-color-be", "border-color-t", "border-color-r", "border-color-b", "border-color-l"],
      "border-color-x": ["border-color-s", "border-color-e", "border-color-r", "border-color-l"],
      "border-color-y": ["border-color-bs", "border-color-be", "border-color-t", "border-color-b"],
      translate: ["translate-x", "translate-y", "translate-none"],
      "translate-none": ["translate", "translate-x", "translate-y", "translate-z"],
      "scroll-m": ["scroll-mx", "scroll-my", "scroll-ms", "scroll-me", "scroll-mbs", "scroll-mbe", "scroll-mt", "scroll-mr", "scroll-mb", "scroll-ml"],
      "scroll-mx": ["scroll-ms", "scroll-me", "scroll-mr", "scroll-ml"],
      "scroll-my": ["scroll-mbs", "scroll-mbe", "scroll-mt", "scroll-mb"],
      "scroll-p": ["scroll-px", "scroll-py", "scroll-ps", "scroll-pe", "scroll-pbs", "scroll-pbe", "scroll-pt", "scroll-pr", "scroll-pb", "scroll-pl"],
      "scroll-px": ["scroll-ps", "scroll-pe", "scroll-pr", "scroll-pl"],
      "scroll-py": ["scroll-pbs", "scroll-pbe", "scroll-pt", "scroll-pb"],
      touch: ["touch-x", "touch-y", "touch-pz"],
      "touch-x": ["touch"],
      "touch-y": ["touch"],
      "touch-pz": ["touch"]
    },
    conflictingClassGroupModifiers: {
      "font-size": ["leading"]
    },
    postfixLookupClassGroups: ["container-type"],
    orderSensitiveModifiers: ["*", "**", "after", "backdrop", "before", "details-content", "file", "first-letter", "first-line", "marker", "placeholder", "selection"]
  };
}, Ja = /* @__PURE__ */ Aa(qa);
function bi(...e) {
  return Ja(ni(e));
}
const jr = (e) => typeof e == "boolean" ? `${e}` : e === 0 ? "0" : e, $r = ni, Ya = (e, t) => (n) => {
  var s;
  if (t?.variants == null) return $r(e, n?.class, n?.className);
  const { variants: r, defaultVariants: o } = t, i = Object.keys(r).map((u) => {
    const f = n?.[u], h = o?.[u];
    if (f === null) return null;
    const b = jr(f) || jr(h);
    return r[u][b];
  }), l = n && Object.entries(n).reduce((u, f) => {
    let [h, b] = f;
    return b === void 0 || (u[h] = b), u;
  }, {}), c = t == null || (s = t.compoundVariants) === null || s === void 0 ? void 0 : s.reduce((u, f) => {
    let { class: h, className: b, ...w } = f;
    return Object.entries(w).every((L) => {
      let [C, $] = L;
      return Array.isArray($) ? $.includes({
        ...o,
        ...l
      }[C]) : {
        ...o,
        ...l
      }[C] === $;
    }) ? [
      ...u,
      h,
      b
    ] : u;
  }, []);
  return $r(e, i, c, n?.class, n?.className);
}, Xa = Ya(
  "inline-flex items-center justify-center gap-2 whitespace-nowrap rounded-md text-sm font-medium transition-all active:translate-y-px disabled:pointer-events-none disabled:opacity-50 shrink-0 outline-none focus-visible:border-ring focus-visible:ring-ring/50 focus-visible:ring-3 border border-transparent",
  {
    variants: {
      variant: {
        default: "bg-primary text-primary-foreground btn-raised [--raised-tint:var(--primary)] hover:bg-primary/90",
        destructive: "bg-destructive text-white btn-raised [--raised-tint:var(--destructive)] hover:bg-destructive/90",
        outline: "bg-background btn-raised-soft hover:bg-accent hover:text-accent-foreground dark:bg-input/30",
        secondary: "bg-secondary text-secondary-foreground btn-raised-soft hover:bg-secondary/80",
        ghost: "hover:bg-background hover:text-accent-foreground",
        link: "active:translate-y-0 text-primary underline-offset-4 hover:underline"
      },
      size: {
        default: "h-9 px-4 py-2",
        xs: "h-6 gap-1 rounded-md px-2 text-xs",
        sm: "h-8 rounded-md gap-1.5 px-3",
        lg: "h-10 rounded-md px-6"
      }
    },
    defaultVariants: {
      variant: "default",
      size: "default"
    }
  }
), Qa = ["type"], fs = /* @__PURE__ */ Hs({
  __name: "Button",
  props: {
    variant: {},
    size: {},
    class: {},
    type: { default: "button" }
  },
  setup(e) {
    const t = e;
    return (n, s) => (le(), be("button", {
      type: t.type,
      class: yt(Ie(bi)(Ie(Xa)({ variant: t.variant, size: t.size }), t.class))
    }, [
      Eo(n.$slots, "default")
    ], 10, Qa));
  }
}), Nr = /* @__PURE__ */ Hs({
  __name: "Card",
  props: {
    class: {}
  },
  setup(e) {
    const t = e;
    return (n, s) => (le(), be("div", {
      "data-slot": "card",
      class: yt(
        Ie(bi)(
          "bg-card text-card-foreground flex flex-col gap-6 rounded-xl border py-6 shadow-sm",
          t.class
        )
      )
    }, [
      Eo(n.$slots, "default")
    ], 2));
  }
}), Fr = { error: 0, warning: 1 };
function pe(e, t, n, s, r = {}) {
  return { rule: e, severity: t, page: n, message: s, ...r };
}
function Za(e) {
  return [...e].sort((t, n) => {
    const s = Fr[t.severity] - Fr[n.severity];
    return s !== 0 ? s : t.rule.localeCompare(n.rule);
  });
}
const eu = 60, tu = 160;
function nu(e) {
  const t = (e.querySelector("title")?.textContent ?? "").trim(), n = (e.querySelector('meta[name="description"]')?.getAttribute("content") ?? "").trim(), s = (e.querySelector('link[rel="canonical"]')?.getAttribute("href") ?? "").trim(), r = (e.querySelector('meta[name="robots"]')?.getAttribute("content") ?? "").trim(), o = (e.querySelector("html")?.getAttribute("lang") ?? "").trim(), i = [...e.querySelectorAll("h1")], l = (i[0]?.textContent ?? "").trim(), c = ds(e, "og:title"), u = ds(e, "og:description"), f = ds(e, "og:image"), h = [], b = /* @__PURE__ */ new Set();
  for (const w of e.querySelectorAll('script[type="application/ld+json"]')) {
    const L = (w.textContent ?? "").trim();
    if (L)
      try {
        for (const C of su(JSON.parse(L)))
          b.has(C) || (b.add(C), h.push(C));
      } catch {
      }
  }
  return {
    title: t,
    titleLength: t.length,
    description: n,
    descriptionLength: n.length,
    canonical: s,
    robots: r || "index, follow",
    lang: o,
    h1: l,
    h1Count: i.length,
    ogTitle: c,
    ogDescription: u,
    ogImage: f,
    jsonLdTypes: h
  };
}
function ds(e, t) {
  return (e.querySelector(`meta[property="${t}"]`)?.getAttribute("content") ?? "").trim();
}
function su(e) {
  const t = [];
  return As(e, t), t;
}
function As(e, t) {
  if (e == null || typeof e != "object") return;
  if (Array.isArray(e)) {
    for (const r of e) As(r, t);
    return;
  }
  const n = (
    /** @type {Record<string, unknown>} */
    e
  ), s = n["@type"];
  if (typeof s == "string")
    t.push(s);
  else if (Array.isArray(s))
    for (const r of s)
      typeof r == "string" && t.push(r);
  Array.isArray(n["@graph"]) && As(n["@graph"], t);
}
const zr = 60, Dr = 160, ru = 50 * 1024, ou = /* @__PURE__ */ new Set([
  "Organization",
  "WebSite",
  "WebPage",
  "Article",
  "BlogPosting",
  "FAQPage",
  "HowTo",
  "BreadcrumbList"
]);
function iu(e, { path: t }) {
  const n = [], r = (e.querySelector("title")?.textContent ?? "").trim();
  r ? r.length > zr && n.push(
    pe(
      "title-length",
      "warning",
      t,
      `The title is ${r.length} characters; keep it under ${zr}.`
    )
  ) : n.push(
    pe("title-missing", "error", t, "This page has no title.")
  );
  const i = (e.querySelector('meta[name="description"]')?.getAttribute("content") ?? "").trim();
  i ? i.length > Dr && n.push(
    pe(
      "description-length",
      "warning",
      t,
      `The description is ${i.length} characters; keep it under ${Dr}.`
    )
  ) : n.push(
    pe(
      "description-missing",
      "warning",
      t,
      "This page has no description."
    )
  );
  const l = e.querySelectorAll("h1");
  l.length === 0 ? n.push(
    pe(
      "heading",
      "error",
      t,
      "This page has no main heading."
    )
  ) : l.length > 1 && n.push(
    pe(
      "heading",
      "warning",
      t,
      `This page has ${l.length} main headings; keep one.`,
      { count: l.length }
    )
  );
  const c = [...e.querySelectorAll("img")].filter(
    (N) => !N.hasAttribute("alt")
  );
  if (c.length > 0) {
    const N = c.length;
    n.push(
      pe(
        "image-alt",
        "warning",
        t,
        N === 1 ? "1 image has no alt text." : `${N} images have no alt text.`,
        { count: N }
      )
    );
  }
  (e.querySelector("html")?.getAttribute("lang") ?? "").trim() || n.push(
    pe(
      "html-lang",
      "warning",
      t,
      "The page doesn't declare a language."
    )
  );
  const h = ["og:title", "og:description", "og:image"].filter(
    (N) => !e.querySelector(`meta[property="${N}"]`)
  );
  h.length > 0 && n.push(
    pe(
      "social-preview",
      "warning",
      t,
      "This page has no social preview tags.",
      { detail: `Missing ${h.join(", ")}.` }
    )
  ), (e.querySelector('meta[name="robots"]')?.getAttribute("content") ?? "").toLowerCase().includes("noindex") && n.push(
    pe(
      "noindex",
      "warning",
      t,
      "Search engines are told not to index this page."
    )
  ), e.querySelector('link[rel="canonical"]')?.getAttribute("href")?.trim() || n.push(
    pe(
      "canonical-missing",
      "warning",
      t,
      "This page has no canonical URL."
    )
  );
  const L = [...e.querySelectorAll('script[type="application/ld+json"]')], C = [];
  let $ = !1;
  if (L.length === 0)
    n.push(
      pe(
        "json-ld-missing",
        "warning",
        t,
        "This page has no structured data (JSON-LD)."
      )
    );
  else {
    for (const N of L) {
      const G = (N.textContent ?? "").trim();
      if (G)
        try {
          C.push(JSON.parse(G));
        } catch {
          $ = !0;
        }
    }
    $ ? n.push(
      pe(
        "json-ld-invalid",
        "error",
        t,
        "Structured data on this page is not valid JSON."
      )
    ) : C.length > 0 && !lu(C) && n.push(
      pe(
        "json-ld-type",
        "warning",
        t,
        "Structured data does not include a useful schema type."
      )
    );
  }
  e.querySelector("details") !== null && !cu(C, "FAQPage") && n.push(
    pe(
      "faq-structure",
      "warning",
      t,
      "This page looks like FAQ content but has no FAQPage structured data."
    )
  );
  const j = [...e.querySelectorAll("img")].filter((N) => {
    const G = N.getAttribute("width"), A = N.getAttribute("height");
    return !(G?.trim() && A?.trim());
  });
  if (j.length > 0) {
    const N = j.length;
    n.push(
      pe(
        "img-dimensions",
        "warning",
        t,
        N === 1 ? "1 image is missing width and height." : `${N} images are missing width and height.`,
        { count: N }
      )
    );
  }
  const O = [...e.querySelectorAll("head script[src]")].filter((N) => !(N.getAttribute("defer") != null || N.getAttribute("async") != null || (N.getAttribute("type") ?? "").toLowerCase() === "module"));
  O.length > 0 && n.push(
    pe(
      "sync-script",
      "warning",
      t,
      "A render-blocking script in the page head may slow loading.",
      { count: O.length }
    )
  );
  const B = [];
  for (const N of e.querySelectorAll("script:not([src]), style"))
    new TextEncoder().encode(N.textContent ?? "").length > ru && B.push(N);
  return B.length > 0 && n.push(
    pe(
      "huge-inline",
      "warning",
      t,
      "Large inline script or style may slow the page.",
      { count: B.length }
    )
  ), n;
}
function lu(e) {
  for (const t of e)
    if (au(t)) return !0;
  return !1;
}
function cu(e, t) {
  for (const n of e)
    if (uu(n, t)) return !0;
  return !1;
}
function au(e) {
  return yi(e).some((t) => ou.has(t));
}
function uu(e, t) {
  return yi(e).includes(t);
}
function yi(e) {
  const t = [];
  return Ts(e, t), t;
}
function Ts(e, t) {
  if (e == null || typeof e != "object") return;
  if (Array.isArray(e)) {
    for (const r of e) Ts(r, t);
    return;
  }
  const n = (
    /** @type {Record<string, unknown>} */
    e
  ), s = n["@type"];
  if (typeof s == "string")
    t.push(s);
  else if (Array.isArray(s))
    for (const r of s)
      typeof r == "string" && t.push(r);
  Array.isArray(n["@graph"]) && Ts(n["@graph"], t);
}
const fu = { class: "flex shrink-0 items-center justify-between gap-2 px-3" }, du = {
  class: "text-muted-foreground shrink-0 px-3 text-[0.72rem]",
  "data-orso-checks-summary": ""
}, hu = { class: "flex min-h-0 flex-1 flex-col gap-3 overflow-auto px-3 pb-0.5" }, pu = {
  key: 0,
  class: "border-border m-0 flex list-none flex-col gap-2 border-b pb-3",
  "data-orso-checks-meta": ""
}, gu = { class: "flex items-baseline justify-between gap-2" }, mu = { class: "text-muted-foreground text-[0.68rem] font-medium tracking-wide uppercase" }, bu = {
  key: 0,
  class: "text-muted-foreground text-[0.68rem] tabular-nums"
}, yu = {
  class: "m-0 flex list-none flex-col gap-2",
  "data-orso-checks-findings": ""
}, xu = {
  key: 0,
  class: "text-muted-foreground px-0.5 py-1 text-sm"
}, vu = ["data-rule"], _u = { class: "flex items-start gap-2.5 px-3 py-2.5" }, wu = ["aria-label"], ku = { class: "min-w-0 flex-1" }, Su = { class: "text-sm leading-snug" }, Cu = {
  key: 0,
  class: "text-muted-foreground mt-1 text-[0.72rem]"
}, Au = {
  key: 0,
  class: "inline-flex items-center gap-1 text-destructive"
}, Tu = {
  key: 1,
  class: "inline-flex items-center gap-1 text-[color-mix(in_oklch,var(--warning)_90%,white)]"
}, Eu = "__orso-picker", Pu = "orso:preview", Ou = /* @__PURE__ */ Hs({
  __name: "ChecksPanel",
  setup(e) {
    const t = /* @__PURE__ */ Lt(!1), n = /* @__PURE__ */ Lt(!1), s = /* @__PURE__ */ Lt(!1), r = /* @__PURE__ */ Lt([]), o = /* @__PURE__ */ Lt(""), i = /* @__PURE__ */ Lt(null);
    function l() {
      const v = `${location.pathname}${location.search}`;
      return v === "" ? "/" : v;
    }
    function c(v) {
      return v < 1024 ? `${v} B` : v < 1024 * 1024 ? `${(v / 1024).toFixed(1)} KB` : `${(v / (1024 * 1024)).toFixed(1)} MB`;
    }
    function u(v) {
      return {
        scripts: v.querySelectorAll("script").length,
        styles: v.querySelectorAll('link[rel="stylesheet"], style').length,
        images: v.querySelectorAll("img").length,
        htmlBytes: new TextEncoder().encode(v.documentElement.outerHTML).length
      };
    }
    function f(v) {
      const U = [`The preview checks found an issue on ${v.page || l()}: ${v.message}`];
      return v.detail && U.push(v.detail), U.push(
        "",
        "Fix it in the site and keep the existing copy and design.",
        "Rely on HMR; do not run a build or publish."
      ), U.join(`
`);
    }
    function h(v) {
      window.parent !== window && window.parent.postMessage({ type: Pu, op: "fix", text: v }, "*");
    }
    function b() {
      const v = iu(document, { path: l() });
      r.value = Za(v), i.value = nu(document);
      const { scripts: se, styles: U, images: Ae, htmlBytes: tt } = u(document);
      o.value = `${se} scripts · ${U} styles · ${Ae} images · HTML ${c(tt)}`, n.value = !0;
    }
    function w(v) {
      t.value = v, v && !n.value && b();
    }
    function L() {
      w(!t.value);
    }
    function C() {
      b();
    }
    function $() {
      const v = document.getElementById(Eu) !== null;
      s.value = v, v && t.value && w(!1);
    }
    function D(v) {
      return v.trim() ? v : null;
    }
    function j(v) {
      return v.h1Count === 0 ? null : v.h1Count === 1 ? v.h1 : `${v.h1} (${v.h1Count} H1s)`;
    }
    let W = null;
    Ao(() => {
      W = new MutationObserver($), W.observe(document.documentElement, { childList: !0, subtree: !0 }), $(), b();
    }), To(() => W?.disconnect());
    const O = Ft(() => r.value), B = Ft(() => r.value.filter((v) => v.severity === "error").length), N = Ft(() => r.value.filter((v) => v.severity === "warning").length), G = Ft(() => {
      if (!n.value) return "Page details";
      const v = [];
      return B.value && v.push(`${B.value} error${B.value === 1 ? "" : "s"}`), N.value && v.push(`${N.value} warning${N.value === 1 ? "" : "s"}`), v.length === 0 ? "Page details" : v.join(", ");
    }), A = Ft(() => {
      const v = i.value;
      return v ? [
        {
          key: "title",
          label: "Title",
          value: D(v.title),
          hint: `${v.titleLength}/${eu}`
        },
        {
          key: "description",
          label: "Description",
          value: D(v.description),
          hint: `${v.descriptionLength}/${tu}`
        },
        { key: "canonical", label: "Canonical", value: D(v.canonical) },
        { key: "robots", label: "Robots", value: D(v.robots) },
        { key: "lang", label: "Language", value: D(v.lang) },
        { key: "h1", label: "H1", value: j(v) },
        { key: "ogTitle", label: "OG title", value: D(v.ogTitle) },
        { key: "ogDescription", label: "OG description", value: D(v.ogDescription) },
        { key: "ogImage", label: "OG image", value: D(v.ogImage) },
        {
          key: "jsonLd",
          label: "Structured data",
          value: v.jsonLdTypes.length > 0 ? v.jsonLdTypes.join(", ") : null
        }
      ] : [];
    });
    return (v, se) => (le(), be("div", {
      class: yt(["pointer-events-auto flex flex-col items-end gap-2", { "pointer-events-none opacity-35": s.value }]),
      "data-testid": "orso-checks-panel"
    }, [
      ul(oe(Nr, {
        class: "border-border flex w-[min(22rem,calc(100vw-24px))] max-h-[min(32rem,calc(100vh-80px))] flex-col gap-3 overflow-hidden rounded-xl border py-3 shadow-lg",
        "data-orso-checks-drawer": ""
      }, {
        default: Nt(() => [
          Ee("div", fu, [
            se[1] || (se[1] = Ee("div", { class: "text-sm font-medium tracking-tight" }, "Page details", -1)),
            oe(fs, {
              variant: "outline",
              size: "xs",
              "data-orso-checks-refresh": "",
              onClick: C
            }, {
              default: Nt(() => [
                oe(Ie(Zc), {
                  class: "size-3",
                  "aria-hidden": "true"
                }),
                se[0] || (se[0] = gt(" Refresh ", -1))
              ]),
              _: 1
            })
          ]),
          Ee("div", du, lt(o.value), 1),
          Ee("div", hu, [
            A.value.length ? (le(), be("dl", pu, [
              (le(!0), be(fe, null, ir(A.value, (U) => (le(), be("div", {
                key: U.key,
                class: "grid gap-0.5"
              }, [
                Ee("div", gu, [
                  Ee("dt", mu, lt(U.label), 1),
                  U.hint ? (le(), be("span", bu, lt(U.hint), 1)) : jt("", !0)
                ]),
                Ee("dd", {
                  class: yt(["m-0 text-sm leading-snug break-words", U.value ? "" : "text-muted-foreground"])
                }, lt(U.value ?? "Missing"), 3)
              ]))), 128))
            ])) : jt("", !0),
            Ee("ul", yu, [
              O.value.length === 0 ? (le(), be("li", xu, " No issues found on this page. ")) : jt("", !0),
              (le(!0), be(fe, null, ir(O.value, (U) => (le(), be("li", {
                key: `${U.rule}:${U.message}`,
                "data-rule": U.rule
              }, [
                oe(Nr, { class: "border-border bg-card gap-0 rounded-lg border py-0 shadow-sm" }, {
                  default: Nt(() => [
                    Ee("div", _u, [
                      Ee("div", {
                        class: yt([
                          "mt-0.5 flex size-7 shrink-0 items-center justify-center rounded-md border",
                          U.severity === "error" ? "border-destructive/30 bg-destructive/15 text-destructive" : "border-[color-mix(in_oklch,var(--warning)_35%,transparent)] bg-[color-mix(in_oklch,var(--warning)_18%,transparent)] text-[color-mix(in_oklch,var(--warning)_90%,white)]"
                        ]),
                        "aria-label": U.severity
                      }, [
                        U.severity === "error" ? (le(), dn(Ie(Tr), {
                          key: 0,
                          class: "size-3.5"
                        })) : (le(), dn(Ie(Er), {
                          key: 1,
                          class: "size-3.5"
                        }))
                      ], 10, wu),
                      Ee("div", ku, [
                        Ee("div", Su, lt(U.message), 1),
                        U.detail ? (le(), be("div", Cu, lt(U.detail), 1)) : jt("", !0),
                        oe(fs, {
                          size: "xs",
                          class: "mt-2",
                          "data-orso-checks-fix": U.rule,
                          onClick: (Ae) => h(f(U))
                        }, {
                          default: Nt(() => [
                            oe(Ie(na), {
                              class: "size-3",
                              "aria-hidden": "true"
                            }),
                            se[2] || (se[2] = gt(" Fix this ", -1))
                          ]),
                          _: 1
                        }, 8, ["data-orso-checks-fix", "onClick"])
                      ])
                    ])
                  ]),
                  _: 2
                }, 1024)
              ], 8, vu))), 128))
            ])
          ])
        ]),
        _: 1
      }, 512), [
        [vc, t.value]
      ]),
      oe(fs, {
        variant: "outline",
        size: "sm",
        class: "rounded-full shadow-lg",
        "data-orso-checks-trigger": "",
        "aria-expanded": t.value,
        "aria-label": t.value ? "Close" : G.value,
        onClick: L
      }, {
        default: Nt(() => [
          t.value ? (le(), be(fe, { key: 0 }, [
            oe(Ie(ra), {
              class: "size-3.5",
              "aria-hidden": "true"
            }),
            se[3] || (se[3] = gt(" Close ", -1))
          ], 64)) : n.value ? B.value === 0 && N.value === 0 ? (le(), be(fe, { key: 2 }, [
            oe(Ie(Yc), {
              class: "size-3.5 text-emerald-500",
              "aria-hidden": "true"
            }),
            se[4] || (se[4] = gt(" Page details ", -1))
          ], 64)) : (le(), be(fe, { key: 3 }, [
            B.value > 0 ? (le(), be("span", Au, [
              oe(Ie(Tr), {
                class: "size-3.5",
                "aria-hidden": "true"
              }),
              gt(" " + lt(B.value), 1)
            ])) : jt("", !0),
            N.value > 0 ? (le(), be("span", Tu, [
              oe(Ie(Er), {
                class: "size-3.5",
                "aria-hidden": "true"
              }),
              gt(" " + lt(N.value), 1)
            ])) : jt("", !0)
          ], 64)) : (le(), be(fe, { key: 1 }, [
            gt(" Page details ")
          ], 64))
        ]),
        _: 1
      }, 8, ["aria-expanded", "aria-label"])
    ], 2));
  }
}), Hr = "orso-checks-panel-host";
function Wr() {
  document.querySelectorAll(`[data-testid="${Hr}"]`).forEach((r) => r.remove());
  const e = document.createElement("div");
  e.dataset.testid = Hr, e.setAttribute("data-orso-checks", "host"), Object.assign(e.style, {
    position: "fixed",
    right: "12px",
    bottom: "12px",
    zIndex: "2147483646"
  });
  const t = e.attachShadow({ mode: "open" }), n = document.createElement("style");
  n.textContent = typeof __ORSO_CHECKS_CSS__ == "string" ? __ORSO_CHECKS_CSS__ : "", t.append(n);
  const s = document.createElement("div");
  s.className = "orso-checks-root", t.append(s), document.documentElement.append(e), Dc(Ou).mount(s);
}
typeof document < "u" && (document.readyState === "loading" ? document.addEventListener("DOMContentLoaded", Wr, { once: !0 }) : Wr());
