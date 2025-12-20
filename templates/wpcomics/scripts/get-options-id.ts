Array.from(temp1.querySelectorAll(".genre-item")).map((i) =>
  i.querySelector("span").getAttribute("data-id")
);
