public class Sealed {
    sealed interface Shape permits Sealed.Circle, Sealed.Square {
    }

    record Circle(double radius) implements Shape {
    }

    record Square(double side) implements Shape {
    }

    static double area(Shape shape) {
        if (shape instanceof Circle circle) {
            return Math.PI * circle.radius() * circle.radius();
        }
        if (shape instanceof Square square) {
            return square.side() * square.side();
        }
        return 0;
    }

    public static void main(String[] args) {
        System.out.println(area(new Circle(1)) > 0 && area(new Square(2)) == 4);
    }
}
