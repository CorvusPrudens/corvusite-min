---
title: Mandelbrot Fractal
date: 10/2/23
---

# The Mandelbrot Fractal

Who doesn't love a good fractal?

The Mandelbrot fractal is a classic bit of computational mathematics, and I was surprised to learn that it was only defined and visualized in 1978 (not too long ago!). It's derived from the Mandelbrot set, which is a collection of _complex_ numbers that either diverge or don't diverge given a particular sequence. Those that don't converge are a part of the set. What's so astounding about it is the intricate patterns that arise from surprisingly simple rules.

<div>
  <Mandelbrot />
</div>

## The Math

Those rules are described in essence with this expression:

$$
z_n = z_(n-1)^2 + c, \text{ }z_0 = 0
$$

Each term $z$ in the sequence is equal to the previous term squared plus some value $c$ in the complex plane. For the purposes of visualization, $c$ actually represents the x/y position, where x corresponds to the real component and y to the imaginary. Of course, it would be impractical to keep iterating the sequence until the term gets huge, so you can generally stop if the term's magnitude grows beyond 2.

If you're rusty on complex numbers, recall that they consist of a real component $a$ and an imaginary component $b$ -- $ a + bi $. To get the magnitude (also expressed as absolute value), you'd simply use pythagoras -- $\sqrt{a^2 + b^2}$ -- as if the real and imaginary components form two legs of a right triangle.

Let's look at an example -- say the real component is 0.3 and the imaginary component is 0. After a few iterations, you can see that the value seems to keep growing:

$$
\begin{array}{cccc}
z_1 =& 0^2& + 0.3 + 0i& = 0.3\\
z_2 =& 0.3^2& + 0.3 + 0i& = 0.39\\
z_3 =& 0.39^2& + 0.3 + 0i& \approx 0.45\\
z_4 =& 0.45^2& + 0.3 + 0i& \approx 0.50\\
...\\
z_{12} =& 1.38^2& + 0.3 + 0i& \approx 2.19
\end{array}
$$

and so on. By the twelfth iteration, the magnitude has grown beyond two, so we can pretty safely say this isn't a part of the set.

Let's add a complex component of 0.3:

$$
\begin{array}{cccc}
z_1 =& 0^2& + 0.3 + 0.3i,& |0.3 + 0.3i|& \approx 0.42\\
z_2 =& (0.3 + 0.3i)^2& + 0.3 + 0.3i,& |0.3 + 0.48i|& \approx 0.57\\
z_3 =& (0.3 + 0.48i)^2& + 0.3 + 0.3i,& |0.16 + 0.59i|& \approx 0.61\\
z_4 =& (0.16 + 0.59i)^2& + 0.3 + 0.3i,& |-0.02 + 0.49i|& \approx 0.49\\
...\\
z_{12} =& (0.18 + 0.37)^2& + 0.3 + 0.3i,& |0.20 + 0.43i|& \approx 0.48
\end{array}
$$

In this case, you can see that the sequence actually gets smaller at the fourth term, and by the twelfth term seems to hang around 0.48. In fact, if you run this sequence 1000 times, you'd see the value converge to about 0.44, so this can be counted in the set.

For actual computation, it's not too difficult to create a class that handles complex numbers. Here's my javascript class:

```js
class Complex {
  constructor(x, y) {
    this.a = x;
    this.b = y;
  }

  add(other) {
    this.a += other.a;
    this.b += other.b;
  }

  mult(other) {
    let a = this.a;
    let b = this.b;
    let c = other.a;
    let d = other.b;

    this.a = a * c - b * d;
    this.b = a * d + b * c;
  }
}
```

We only need multiplication and addition for this calculation, so the rest can be ignored. You can easily derive the multiplication by simply working out the operation, keeping in mind the special properties of $i$ --> $i^2 = -1$:

$$
c_1c_2 = (a + bi)(c + di)\\
= ac + adi + bci + bdi^2
$$

rearranged into real and imaginary components:

$$
ac - bd + i(ad + bc)
$$

You can see these correspond exactly to lines 19 and 20 of the above code.

## Iterations

If the magnitude of a coordinate on the complex plane grows beyond 2, then we can safely say it's not in the set. A simple way to go about calculating that is to just run the sequence on every pixel and see if it diverges. A common technique to get interesting coloration is to check how long it took divergent values to diverge, and then map that to a color or brightness.

```js
// Maps an input value from range1 to range2
function map(value, min1, max1, min2, max2) {
  return min2 + (max2 - min2) * ((value - min1) / (max1 - min1));
}

// converts RGB integers to html string representation
function htmlColor(red, green, blue) {
  let number = red.toString(16) + green.toString(16) + blue.toString(16);
  return "#" + number;
}

function getDivergence(c, numIter) {
  let z = new Complex(0, 0);
  for (let n = 0; n < numIter; n++) {
    z.mult(z);
    z.add(c);
    if (z.a * z.a + z.b * z.b > 4) {
      let rbgValue = map(n, 0, numIter, 255, 0);
      return htmlColor(rbgValue, rbgValue, rbgValue);
    }
  }
  // If the value never diverges, then it's in the set.
  // Often this is colored black.
  return "#000000";
}
```

Notice a small optimization at line 19: rather than taking the exact magnitude, we can simply take the square of the magnitude and avoid a square-root operation. Of course, we'd then compare it to the square of 2.

But how many terms in the sequence is appropriate before we determine whether it's in or out? Well, since we don't have infinite computing power, it's usually best to choose a value that maximizes visible detail at the working resolution. Here's an illustration:

At around twenty to thirty iterations per pixel, the view on the left has pretty decent detail (the canvas is 400x400 pixels). The right view, indicated by the little green square, is a bit lacking even at 50 iterations. Something to take away from this is that different levels of zoom and different locations on the fractal will require different iteration counts. To my knowledge there's no foolproof way to calculate how many iterations you need, so it requires a bit of trial and error to get it right.

One cool visualization technique I tried is to take the angle formed by the complex number and map it to a color. With an angle, it's pretty straightforward to use the HSB colorspace. I simply took the angle of the complex number at the point it diverges, and colored non-divergent values the standard black. The result can be quite pleasing in certain positions. You can also have a global angle offset to create trippy visuals:

You might achieve that will something like this:

```js
// ...

// Takes complex number as an angle and maps it to HSB
function getColor(c) {
  let angle = Math.atan2(c.b, c.a);
  let red = map(Math.cos(angle), -1, 1, 0, 255);
  let green = map(Math.cos(angle + Math.PI * 0.666), -1, 1, 0, 255);
  let blue = map(Math.cos(angle - Math.PI * 0.666), -1, 1, 0, 255);
  return htmlColor(red, green, blue);
}

function getDivergence(c, numIter) {
  // ...
  return getColor(z);
  // ...
}
```
