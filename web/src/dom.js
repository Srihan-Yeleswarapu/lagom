// Builds DOM with createElement and text nodes. Nothing here parses HTML, so
// lesson text and compiler output can never inject markup.
//
//   h("p", { className: "lead", onclick: fn }, "text", childNode, null)
//
// Props: className; text (sets textContent); dataset (object); on<event>
// (a function, added as a listener). Other props become attributes: true sets
// a boolean attribute, while false, null and undefined leave it off.
// Children: strings and nodes. null, false and undefined are skipped.
export function h(tag, props = {}, ...children) {
  const node = document.createElement(tag);
  for (const [key, value] of Object.entries(props ?? {})) {
    if (value === undefined || value === null || value === false) continue;
    if (key === "className") node.className = value;
    else if (key === "text") node.textContent = String(value);
    else if (key === "dataset") Object.assign(node.dataset, value);
    else if (key.startsWith("on") && typeof value === "function") {
      node.addEventListener(key.slice(2), value);
    } else node.setAttribute(key, value === true ? "" : String(value));
  }
  for (const child of children.flat()) {
    if (child === undefined || child === null || child === false) continue;
    node.append(child instanceof Node ? child : document.createTextNode(String(child)));
  }
  return node;
}
