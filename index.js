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

  resultsDiv.innerHTML = json.toString();
}

search("glsl function for linear interpolation");
