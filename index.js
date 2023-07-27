console.log("ERROR");

async function search(prompt) {
  const resultsDiv = document.getElementById("results");

  resultsDiv.innerHTML = "";

  const response = await fetch("/api/search", {
    method: "POST",
    headers: {
      "Content-Type": "text/plain",
    },
    body: prompt,
  });

  const json = await response.json();

  resultsDiv.innerHTML = "";

  for ([path, rank] of json) {
    let item = document.createElement("div");
    item.appendChild(document.createTextNode(path));
    item.appendChild(document.createElement("br"));
    resultsDiv.appendChild(item);
  }
}

window.onload = () => {
  let query = document.getElementById("query");

  let currentSearch = Promise.resolve();

  query.addEventListener("keypress", (e) => {
    if (e.key == "Enter") {
      currentSearch.then(() => search(query.value));
    }
  });
};
