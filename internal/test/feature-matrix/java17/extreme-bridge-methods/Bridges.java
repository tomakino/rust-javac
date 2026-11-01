import java.util.function.Supplier;

public class Bridges {
    interface Shape { double area(); }
    interface Named { String name(); }

    abstract static class Base<T> implements Supplier<T> {
        protected final T item;
        Base(T item) { this.item = item; }
        public T get() { return item; }
    }

    static class Sub extends Base<String> {
        Sub(String s) { super(s); }
        @Override public String get() { return item; }
    }

    static class Num implements Comparable<Num> {
        final int value;
        Num(int value) { this.value = value; }
        public int compareTo(Num other) { return Integer.compare(value, other.value); }
    }

    interface Visitor<R> {
        R visitCircle(Circle c);
        R visitSquare(Square s);
    }

    record Circle(double r) {}
    record Square(double side) {}

    static double measure(Visitor<Double> v, Circle c, Square s) {
        return v.visitCircle(c) + v.visitSquare(s);
    }

    static class AreaVisitor implements Visitor<Double> {
        public Double visitCircle(Circle c) { return Math.PI * c.r() * c.r(); }
        public Double visitSquare(Square s) { return s.side() * s.side(); }
    }

    static class Generic<T> {
        T value;
        Generic(T value) { this.value = value; }
        Object id() { return value; }
    }

    static class StringGeneric extends Generic<String> {
        StringGeneric(String value) { super(value); }
        @Override String id() { return value; }
    }

    public static void main(String[] args) {
        Supplier<String> supplier = new Sub("x");
        System.out.println(supplier.get());
        Num a = new Num(1);
        Num b = new Num(2);
        System.out.println(a.compareTo(b));
        System.out.println(measure(new AreaVisitor(), new Circle(1), new Square(2)));
        Generic<String> g = new StringGeneric("y");
        System.out.println(g.id());
    }
}
