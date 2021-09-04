class Point {
  constructor(x, y) {
    this.x = x;
    this.y = y;
  }
}

class RenderComponent {
  /**
   *
   *
   * @param {CanvasRenderingContext2D} canvasCtx
   * @param {number} x, start point x value
   * @param {number} y, start point y value
   * @param {object} context
   * @memberof RenderComponent
   */
  render(canvasCtx, x, y, context) {
    console.error("should impl with method!");
  }
}

const randomColor = () => {
  const r = Math.random() * 255;
  const g = Math.random() * 255;
  const b = Math.random() * 255;
  return `rgb(${r}, ${g}, ${b})`;
};

class Column extends RenderComponent {
  constructor(name, ty) {
    super();
    this.name = name;
    this.ty = ty;
    this.width = 200;
    this.height = 100;
  }

  render(canvasCtx, x, y, context) {
    const color = randomColor();
    canvasCtx.strokeStyle = color;
    canvasCtx.strokeRect(x, y, this.width, this.height);
    canvasCtx.font = "bold 20px serif";
    canvasCtx.fillText(
      `${this.name}: ${this.ty}`,
      x + 20,
      y + this.height / 2,
      this.width
    );
    // draw dots
    const dotsPosition = [
      //left
      { x: x, y: y + this.height / 2 },
      // right
      { x: x + this.width, y: y + this.height / 2 },
    ];
    if (context.isLastColumn) {
      // bottom
      dotsPosition.push({
        x: x + this.width / 2,
        y: y + this.height,
      });
    }
    // if (context.isFirstColumn) {
    //   // top
    //   dotsPosition.push({ x: startPoint.x + this.width / 2, y: startPoint.y });
    // }
    dotsPosition.forEach(({ x, y }) => {
      canvasCtx.beginPath();
      canvasCtx.arc(x, y, 5, 0, 2 * Math.PI);
      canvasCtx.fill();
    });
  }
}

class Entity extends RenderComponent {
  constructor(name, desc, columns) {
    super();
    this.name = name;
    this.desc = desc;
    this.columns = columns;
    this.width = 200;
    this.height = 150;
  }

  render(canvasCtx, x, y, context) {
    const color = randomColor();
    canvasCtx.strokeStyle = color;
    canvasCtx.strokeRect(x, y, this.width, this.height);
    canvasCtx.font = "bold 25px serif";
    canvasCtx.fillText(this.name, x + 20, y + this.height / 2, this.width);
    canvasCtx.font = "20px serif";
    canvasCtx.fillText(this.desc, x + 20, y + this.height / 2 + 25, this.width);

    this.columns.forEach((col, idx) => {
      col.render(canvasCtx, x, y + this.height + idx * 100, {
        isLastColumn: idx == this.columns.length - 1,
        isFirstColumn: idx == 0,
      });
    });
  }

  totalHeight() {
    return this.height + this.columns.map((col) => col.height).sum();
  }

  columnLeftPoint(colName) {
    const colIdx = this.columns.map((col) => col.name).indexOf(colName);
    console.log(colIdx);
    if (colIdx >= 0) {
      return { x: 0, y: this.height + colIdx * 100 + 50 };
    }
  }

  columnRightPoint(colName) {
    const colIdx = this.columns.map((col) => col.name).indexOf(colName);
    console.log(colIdx);
    if (colIdx >= 0) {
      return { x: this.width, y: this.height + colIdx * 100 + 50 };
    }
  }
}

class Relation extends RenderComponent {
  constructor(left, right, ty) {
    super();
    this.left = left;
    this.right = right;
    this.ty = ty;
  }

  render(canvasCtx, startPoint, context) {}
}

function draw() {
  const canvas = document.getElementById("paint");
  const ctx = canvas.getContext("2d");
  const columns1 = [
    new Column("pk", "pk"),
    new Column("name", "text"),
    new Column("age", "int"),
  ];
  const entity1 = new Entity("left", "???", columns1);
  entity1.render(ctx, 100, 100);
  const columns2 = [
    new Column("pk", "pk"),
    new Column("name", "text"),
    new Column("age", "int"),
    new Column("leftId", "int"),
  ];
  const entity2 = new Entity("right", "???", columns2);
  entity2.render(ctx, 400, 100);
  const startPoint = entity1.columnRightPoint("pk");
  const endPoint = entity2.columnLeftPoint("leftId");
  console.log(startPoint, endPoint);
  ctx.beginPath();
  ctx.moveTo(100 + startPoint.x, 100 + startPoint.y);
  ctx.lineTo(400 + endPoint.x, 100 + endPoint.y);
  ctx.stroke();
}
draw();
