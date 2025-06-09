import { e as escape_html } from "../../chunks/escaping.js";
import "clsx";
import { c as pop, p as push } from "../../chunks/index.js";
const replacements = {
  translate: /* @__PURE__ */ new Map([
    [true, "yes"],
    [false, "no"]
  ])
};
function attr(name, value, is_boolean = false) {
  if (is_boolean) return "";
  const normalized = name in replacements && replacements[name].get(value) || value;
  const assignment = is_boolean ? "" : `="${escape_html(normalized, true)}"`;
  return ` ${name}${assignment}`;
}
function _page($$payload, $$props) {
  push();
  let basePath = "";
  console.log(basePath);
  let username = "";
  let password = "";
  $$payload.out += `<meta name="site-url" content="[[SITE_URL]]"/> <h1>Welcome to My Page</h1> <div class="login svelte-949sr8"><h4>Login:</h4> <input type="text" name="username" placeholder="Username"${attr("value", username)}/> <input type="password" name="password" placeholder="Password"${attr("value", password)}/> <div><button>Login</button></div></div>`;
  pop();
}
export {
  _page as default
};
