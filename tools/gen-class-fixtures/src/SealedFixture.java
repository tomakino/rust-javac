public sealed interface SealedFixture permits Circle, Square {
    double area();
}

final class Circle implements SealedFixture {
    public double area() { return 1.0; }
}

non-sealed class Square implements SealedFixture {
    public double area() { return 2.0; }
}
