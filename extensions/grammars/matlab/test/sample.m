% A small tour of MATLAB, for the highlighting sample.
%{
A block comment keeps its markers
on lines of their own.
%}

import matlab.unittest.TestCase

clear all
format long

global DebugLevel
persistent callCount

A = [1 2 3; 4 5 6; 7 8 10];
limits = [eps; pi];
B = A';
C = A.' .* 2.5e-1;
names = {'alpha', 'beta'; "gamma", "delta"};

greeting = 'It''s ready';
message = "Total: ""all""";
fprintf('%d rows\n', size(A, 1));

square = @(x) x.^2;
handler = @disp;
last = A(end, end - 1);
tail = A(2:end, :);

if ~isempty(A) && A(1) > 0
    disp(greeting);
elseif DebugLevel >= 2 || false
    disp(message);
else
    warning('demo:empty', 'Nothing to show');
end

for k = 1:numel(names)
    if k == 2
        continue
    end
    total = square(k) + sum(B(:, k));
end

while total > 1
    total = total / 2;
    if total ~= floor(total)
        break
    end
end

switch lower(names{1})
    case {'alpha', 'beta'}
        callCount = 1;
    otherwise
        callCount = 0;
end

try
    result = inv(C) \ [1; 2; 3];
catch err
    disp(err.message);
end

figure
hold on
plot(1:10, square(1:10), 'LineWidth', 2, ...
    'Color', [0.2 0.4 0.8]);
shapes = Circle(2);
disp(shapes.area());
[~, order] = sort(A(:), 'descend');

function varargout = report(shape, varargin)
    arguments
        shape Circle
    end
    arguments (Repeating)
        varargin
    end
    varargout{1} = shape.Radius;
    return
end

classdef Circle < handle
    properties (Access = private)
        Radius (1,1) double {mustBePositive} = 1
    end

    properties (Dependent)
        Diameter
    end

    methods
        function obj = Circle(radius)
            obj.Radius = radius;
        end

        function value = get.Diameter(obj)
            value = 2 * obj.Radius;
        end

        function value = area(obj)
            value = pi * obj.Radius ^ 2;
        end
    end

    events
        Resized
    end
end
