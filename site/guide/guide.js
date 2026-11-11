const toc = document.getElementById("toc");
const links = [...toc.querySelectorAll("a")];
const heads = links.map(a => document.getElementById(a.getAttribute("href").slice(1)));

// scroll-spy
let current = null;
const spy = () => {
  const y = 90;
  let i = heads.findLastIndex(h => h && h.getBoundingClientRect().top <= y);
  if (i < 0) i = 0;
  if (links[i] === current) return;
  current?.classList.remove("active");
  current = links[i];
  current.classList.add("active");
  toc.querySelectorAll("li.grp.open").forEach(g => g.classList.remove("open"));
  current.closest("li.grp")?.classList.add("open");
  const side = document.getElementById("side");
  const r = current.getBoundingClientRect(), s = side.getBoundingClientRect();
  if (r.top < s.top || r.bottom > s.bottom) current.scrollIntoView({ block: "center" });
};
addEventListener("scroll", spy, { passive: true });
spy();

// filter
const filter = document.getElementById("filter");
filter.addEventListener("input", () => {
  const q = filter.value.trim().toLowerCase();
  toc.classList.toggle("filtering", !!q);
  for (const li of toc.querySelectorAll("li")) {
    const own = li.querySelector(":scope > a").textContent.toLowerCase().includes(q);
    const kid = li.classList.contains("grp") && [...li.querySelectorAll("li")].some(k => k.textContent.toLowerCase().includes(q));
    li.classList.toggle("hide", !!q && !own && !kid);
  }
});

// mobile menu
const menu = document.getElementById("menu"), side = document.getElementById("side");
menu.addEventListener("click", () => menu.setAttribute("aria-expanded", side.classList.toggle("show")));
toc.addEventListener("click", () => { side.classList.remove("show"); menu.setAttribute("aria-expanded", false); });

// copy buttons
for (const b of document.querySelectorAll(".code .copy")) b.addEventListener("click", async () => {
  try { await navigator.clipboard.writeText(b.previousElementSibling.innerText.trimEnd()); b.textContent = "copied"; }
  catch { b.textContent = "failed"; }
  setTimeout(() => (b.textContent = "copy"), 1500);
});
